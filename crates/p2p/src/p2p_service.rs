use std::time::Duration;

use futures::StreamExt;
use libp2p::gossipsub::{self, MessageId, PublishError, TopicHash};
use libp2p::swarm::SwarmEvent;
use libp2p::{noise, tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder};
use tracing::{debug, info, warn};

use crate::behaviour::{TonicBehaviour, TonicBehaviourEvent};
use crate::config::Config;
use crate::gossipsub::{GossipMessage, GossipTopics};

/// Enum representing the events relevant to Tonic blockchain
#[derive(Clone, Debug)]
pub enum TonicP2PEvent {
    #[allow(unused)]
    GossipsubMessage {
        peer_id: PeerId,
        message_id: MessageId,
        topic_hash: TopicHash,
        message: GossipMessage,
    },
}

struct NetworkMetadata {
    topics: GossipTopics,
}

impl NetworkMetadata {
    fn new(p2p_config: &Config) -> Self {
        let topics = GossipTopics::new(&p2p_config.network_name);
        Self { topics }
    }
}

/// [`P2PService`] is the core component of the libp2p network layer.
/// It sits in the center of both any P2P activity including handling
/// incoming and outgoing messages and connections to peers.
pub struct P2PService {
    /// Local peer id
    pub local_peer_id: PeerId,

    /// TCP listen address. Set after [`P2PService::start`]
    /// method is called.
    pub address: Option<Multiaddr>,

    /// libp2p Swarm object
    swarm: Swarm<TonicBehaviour>,

    /// TCP port to listen. Keep in mind if value 0 is provided
    /// the value will be updated on start.
    tcp_port: u16,

    /// Network metadata
    network_metadata: NetworkMetadata,
}

impl P2PService {
    pub fn new(config: Config) -> Self {
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
            tcp_port: config.tcp_port,
            address: None,
            network_metadata,
        }
    }

    pub async fn start(&mut self) {
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

    pub async fn next_event(&mut self) -> Option<TonicP2PEvent> {
        let event = self.swarm.select_next_some().await;
        debug!(?event);

        match event {
            SwarmEvent::Behaviour(event) => self.handle_behaviour_event(event),
            SwarmEvent::ListenerClosed {
                addresses, reason, ..
            } => {
                info!("P2P listener(s) `{addresses:?}` closed with `{reason:?}`");
                None
            }
            _ => None,
        }
    }

    fn handle_behaviour_event(&self, event: TonicBehaviourEvent) -> Option<TonicP2PEvent> {
        match event {
            TonicBehaviourEvent::Gossipsub(event) => self.handle_gossipsub_event(event),
            _ => None,
        }
    }

    fn handle_gossipsub_event(&self, event: gossipsub::Event) -> Option<TonicP2PEvent> {
        match event {
            gossipsub::Event::Message {
                propagation_source,
                message_id,
                message,
            } => {
                let tag = self
                    .network_metadata
                    .topics
                    .get_gossip_tag(&message.topic)?;
                match GossipMessage::deserialize(&tag, &message.data) {
                    Ok(decoded_message) => Some(TonicP2PEvent::GossipsubMessage {
                        peer_id: propagation_source,
                        message_id,
                        topic_hash: message.topic,
                        message: decoded_message,
                    }),
                    Err(err) => {
                        warn!(
                            ?message_id,
                            ?propagation_source,
                            ?message,
                            ?err,
                            "Failed to deserialize gossip message"
                        );
                        None
                    }
                }
            }
            _ => None,
        }
    }

    pub fn publish_message(&mut self, message: GossipMessage) -> Result<MessageId, PublishError> {
        let topic_hash = self
            .network_metadata
            .topics
            .get_topic_hash_from_message(&message);
        let encoded_data = message
            .serialize()
            .map_err(|err| PublishError::TransformFailed(err))?;

        self.swarm
            .behaviour_mut()
            .publish_message(topic_hash, encoded_data)
    }
}
