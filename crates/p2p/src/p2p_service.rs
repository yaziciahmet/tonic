use std::time::Duration;

use futures::StreamExt;
use libp2p::gossipsub::{self, MessageId};
use libp2p::swarm::SwarmEvent;
use libp2p::{noise, tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder};
use tokio::select;
use tokio::sync::mpsc;
use tracing::{debug, error, info, trace, warn};

use crate::behaviour::{TonicBehaviour, TonicBehaviourEvent};
use crate::config::Config;
use crate::gossipsub::{GossipCodec, GossipMessage, GossipTopics};

#[derive(Debug)]
pub enum P2PRequest {
    BroadcastMessage(GossipMessage),
}

const MAX_MESSAGE_SIZE: usize = 1024 * 1024 * 4; // 4 MB

struct NetworkMetadata {
    codec: GossipCodec,
    topics: GossipTopics,
}

impl NetworkMetadata {
    fn new(p2p_config: &Config) -> Self {
        let codec = GossipCodec::new(MAX_MESSAGE_SIZE);
        let topics = GossipTopics::new(&p2p_config.network_name);
        Self { codec, topics }
    }
}

/// [`P2PService`] is the core component of the libp2p network layer.
/// It sits in the center of both any P2P activity including handling
/// incoming and outgoing messages and connections to peers.
pub struct P2PService<B: Broadcast> {
    /// Local peer id
    pub local_peer_id: PeerId,

    /// TCP listen address. Set after [`P2PService::start`]
    /// method is called.
    pub address: Option<Multiaddr>,

    /// libp2p Swarm object
    swarm: Swarm<TonicBehaviour>,

    /// Request receiver channel
    request_receiver: mpsc::Receiver<P2PRequest>,

    /// P2P event broadcaster
    broadcast: B,

    /// TCP port to listen. Keep in mind if value 0 is provided
    /// the value will be updated on start.
    tcp_port: u16,

    /// Network metadata
    network_metadata: NetworkMetadata,
}

impl<B> P2PService<B>
where
    B: Broadcast,
{
    pub fn new(config: Config, request_receiver: mpsc::Receiver<P2PRequest>, broadcast: B) -> Self {
        let swarm = SwarmBuilder::with_existing_identity(config.keypair.clone())
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .expect("Swarm TCP config to be valid")
            .with_behaviour(|_| TonicBehaviour::new(&config))
            .expect("Swarm behaviour construction should succeed")
            .with_swarm_config(|cfg| {
                if let Some(timeout) = config.connection_idle_timeout {
                    cfg.with_idle_connection_timeout(timeout)
                } else {
                    cfg
                }
            })
            .build();

        let local_peer_id = config.keypair.public().to_peer_id();

        let network_metadata = NetworkMetadata::new(&config);

        Self {
            local_peer_id,
            swarm,
            request_receiver,
            broadcast,
            tcp_port: config.tcp_port,
            address: None,
            network_metadata,
        }
    }

    pub async fn listen(&mut self) {
        let listen_addr = format!("/ip4/0.0.0.0/tcp/{}", self.tcp_port)
            .parse()
            .expect("Multiaddress parsing should succeed");

        self.swarm
            .listen_on(listen_addr)
            .expect("Swarm should start listening");

        // Assigned address might differ from the propagated address
        let new_listen_addr =
            tokio::time::timeout(Duration::from_secs(5), self.await_listen_address())
                .await
                .expect("P2PService should get a new listen address in 5 seconds");
        self.address = Some(new_listen_addr);

        info!(
            "The p2p service started on `{}` with `{}`",
            self.address.as_ref().unwrap(),
            self.local_peer_id
        );
    }

    async fn await_listen_address(&mut self) -> Multiaddr {
        loop {
            if let SwarmEvent::NewListenAddr { address, .. } = self.swarm.select_next_some().await {
                return address;
            }
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields (local_peer_id = %self.local_peer_id, address = %self.address.as_ref().unwrap()))]
    pub async fn run(&mut self) {
        loop {
            select! {
                event = self.swarm.select_next_some() => {
                    trace!(?event);
                    #[allow(clippy::single_match)]
                    match event {
                        SwarmEvent::Behaviour(event) => {
                            if let Err(err) = self.handle_behaviour_event(event) {
                                error!("Failed to handle behaviour event: {}", err);
                            }
                        }
                        _ => {},
                    }
                }
                request = self.request_receiver.recv() => {
                    let request = request.expect("P2P request should not be empty");
                    trace!(?request);
                    match request {
                        P2PRequest::BroadcastMessage(message) => {
                            if let Err(err) = self.publish_message(message) {
                                error!("Failed to broadcast gossip message: {}", err);
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_behaviour_event(&self, event: TonicBehaviourEvent) -> anyhow::Result<()> {
        match event {
            TonicBehaviourEvent::Gossipsub(event) => self.handle_gossipsub_event(event),
            _ => Ok(()),
        }
    }

    fn handle_gossipsub_event(&self, event: gossipsub::Event) -> anyhow::Result<()> {
        match event {
            gossipsub::Event::Message {
                propagation_source,
                message_id,
                message,
            } => {
                let Some(tag) = self.network_metadata.topics.get_gossip_tag(&message.topic) else {
                    // TODO: lower peer score
                    return Ok(());
                };
                match self.network_metadata.codec.deserialize(&tag, &message.data) {
                    Ok(decoded_message) => {
                        self.handle_gossipsub_message(decoded_message, propagation_source)
                    }
                    Err(err) => {
                        warn!(
                            ?message_id,
                            ?propagation_source,
                            ?message,
                            ?err,
                            "Failed to deserialize gossip message"
                        );
                        // TODO: lower peer score
                        Ok(())
                    }
                }
            }
            _ => Ok(()),
        }
    }

    fn handle_gossipsub_message(
        &self,
        message: GossipMessage,
        peer_id: PeerId,
    ) -> anyhow::Result<()> {
        debug!(gossip_message = ?message, ?peer_id);
        match message {
            GossipMessage::Dummy(value) => self.broadcast.broadcast_dummy((value, peer_id)),
        }
    }

    fn publish_message(&mut self, message: GossipMessage) -> anyhow::Result<MessageId> {
        let topic_hash = self
            .network_metadata
            .topics
            .get_topic_hash_from_message(&message);
        let encoded_data = self.network_metadata.codec.serialize(&message)?;

        Ok(self
            .swarm
            .behaviour_mut()
            .publish_message(topic_hash, encoded_data)?)
    }
}

pub type IncomingDummyMessage = (u64, PeerId);

pub trait Broadcast {
    fn broadcast_dummy(&self, data: IncomingDummyMessage) -> anyhow::Result<()>;
}
