use std::time::Duration;

use libp2p::futures::StreamExt;
use libp2p::gossipsub::{self, MessageId, PublishError};
use libp2p::swarm::SwarmEvent;
use libp2p::{noise, tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder};

use crate::behaviour::{TonicBehaviour, TonicBehaviourEvent};
use crate::config::Config;
use crate::gossipsub::{GossipMessage, GossipTopics};

struct NetworkMetadata {
    topics: GossipTopics,
}

impl NetworkMetadata {
    fn new(p2p_config: &Config) -> Self {
        let topics = GossipTopics::new(&p2p_config.network_name);
        Self { topics }
    }
}

pub struct P2PService {
    /// Local peer id
    pub local_peer_id: PeerId,

    swarm: Swarm<TonicBehaviour>,
    tcp_port: u16,
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
            .expect("Swarm behaviour construction to succeed")
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
            network_metadata,
        }
    }

    pub async fn start(&mut self) {
        let peer_id = self.local_peer_id;
        let multiaddr = format!("/ip4/0.0.0.0/tcp/{}", self.tcp_port)
            .parse::<Multiaddr>()
            .expect("Multiaddress parsing to succeed");

        tracing::info!("The p2p service starts on the `{multiaddr}` with `{peer_id}`");

        self.swarm
            .listen_on(multiaddr)
            .expect("Swarm to start listening");

        tokio::time::timeout(Duration::from_secs(5), self.await_listen_address())
            .await
            .expect("P2PService to get a new listen address");
    }

    async fn await_listen_address(&mut self) {
        loop {
            if let SwarmEvent::NewListenAddr { .. } = self.swarm.select_next_some().await {
                break;
            }
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

    pub async fn next_event(&mut self) -> Option<TonicBehaviourEvent> {
        let event = self.swarm.select_next_some().await;
        tracing::debug!(?event);

        match event {
            SwarmEvent::Behaviour(event) => self.handle_behaviour_event(event),
            SwarmEvent::ListenerClosed {
                addresses, reason, ..
            } => {
                tracing::info!("p2p listener(s) `{addresses:?}` closed with `{reason:?}`");
                None
            }
            _ => None,
        }
    }

    fn handle_behaviour_event(
        &mut self,
        event: TonicBehaviourEvent,
    ) -> Option<TonicBehaviourEvent> {
        match event {
            TonicBehaviourEvent::Gossipsub(event) => self.handle_gossipsub_event(event),
            _ => None,
        }
    }

    fn handle_gossipsub_event(&mut self, event: gossipsub::Event) -> Option<TonicBehaviourEvent> {
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
                    Ok(_message) => {
                        // TODO: Implement a custom TonicBehaviourEvent, and handle return here
                        None
                    }
                    Err(err) => {
                        tracing::warn!(
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
}

#[cfg(test)]
mod tests {
    use libp2p::identity::Keypair;

    use crate::config::Config;

    use super::P2PService;

    #[tokio::test]
    pub async fn test_start_p2p_service() {
        tonic::initialize_tracing(tracing::Level::TRACE);
        let config1 = Config {
            keypair: Keypair::generate_ed25519(),
            network_name: "testnet".to_owned(),
            tcp_port: 10001,
            connection_idle_timeout: None,
            bootstrap_nodes: vec![],
        };
        let mut node1_p2p = P2PService::new(config1.clone());

        node1_p2p.start().await;

        let node1_multiaddr = format!(
            "/ip4/127.0.0.1/tcp/{}/p2p/{}",
            config1.tcp_port, node1_p2p.local_peer_id
        )
        .parse()
        .unwrap();

        let config2 = Config {
            keypair: Keypair::generate_ed25519(),
            network_name: "testnet".to_owned(),
            tcp_port: 10002,
            connection_idle_timeout: None,
            bootstrap_nodes: vec![node1_multiaddr],
        };
        let mut node2_p2p = P2PService::new(config2);

        node2_p2p.start().await;
    }
}
