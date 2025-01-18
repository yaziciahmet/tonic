use std::time::Duration;

use futures::StreamExt;
use libp2p::gossipsub::{self, MessageId, TopicHash};
use libp2p::swarm::SwarmEvent;
use libp2p::{noise, tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder};
use tokio::select;
use tokio::sync::mpsc;
use tracing::{error, info, trace, warn};

use crate::behaviour::{TonicBehaviour, TonicBehaviourEvent};
use crate::config::Config;

#[derive(Debug)]
pub enum P2PRequest {
    Broadcast(TopicHash, Vec<u8>),
}

/// [`P2PService`] is the core component of the libp2p network layer.
/// It sits in the center of both any P2P activity including handling
/// incoming and outgoing messages and connections to peers.
pub struct P2PService<R: Relayer> {
    /// Local peer id
    pub local_peer_id: PeerId,

    /// TCP listen address. Set after [`P2PService::start`]
    /// method is called.
    pub address: Option<Multiaddr>,

    /// libp2p Swarm object
    swarm: Swarm<TonicBehaviour>,

    /// Request receiver channel
    request_receiver: mpsc::Receiver<P2PRequest>,

    /// P2P event relayer
    relayer: R,

    /// TCP port to listen. Keep in mind if value 0 is provided
    /// the value will be updated on start.
    tcp_port: u16,
}

impl<R> P2PService<R>
where
    R: Relayer,
{
    pub fn new(config: Config, request_receiver: mpsc::Receiver<P2PRequest>, relayer: R) -> Self {
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

        Self {
            local_peer_id,
            swarm,
            request_receiver,
            relayer,
            tcp_port: config.tcp_port,
            address: None,
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

    pub async fn run(mut self) {
        loop {
            select! {
                event = self.swarm.select_next_some() => {
                    trace!("Received event from peer: {:?}", event);
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
                Some(request) = self.request_receiver.recv() => {
                    trace!("Received request: {:?}", request);
                    match request {
                        P2PRequest::Broadcast(topic_hash, data) => {
                            if let Err(err) = self.publish_message(topic_hash, data) {
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
                message,
                ..
            } => {
                if let Err(err) = self.relayer.relay_message(message.topic, message.data) {
                    warn!(
                        "Received malformed message. err = {} peer = {:?} propagating_peer = {:?}",
                        err, message.source, propagation_source
                    );
                    // TODO: report peer
                }

                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn publish_message(
        &mut self,
        topic_hash: TopicHash,
        data: Vec<u8>,
    ) -> anyhow::Result<MessageId> {
        Ok(self
            .swarm
            .behaviour_mut()
            .publish_message(topic_hash, data)?)
    }
}

pub trait Relayer {
    /// Relay the incoming P2P message to interested application components.
    /// Should return error if message is malformed.
    fn relay_message(&self, topic_hash: TopicHash, data: Vec<u8>) -> anyhow::Result<()>;
}
