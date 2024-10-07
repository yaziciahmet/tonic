use std::time::Duration;

use libp2p::futures::StreamExt;
use libp2p::gossipsub::{MessageId, PublishError};
use libp2p::swarm::SwarmEvent;
use libp2p::{noise, tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder};

use crate::behaviour::TonicBehaviour;
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
        let data = message
            .serialize()
            .map_err(|err| PublishError::TransformFailed(err))?;

        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic_hash, data)
    }
}

#[cfg(test)]
mod tests {
    use libp2p::identity::Keypair;

    use crate::config::Config;

    use super::P2PService;

    #[tokio::test]
    pub async fn test_start_p2p_service() {
        tonic::initialize_tracing(tracing::Level::DEBUG);
        let config1 = Config {
            keypair: Keypair::generate_ed25519(),
            network_name: "testnet".to_owned(),
            tcp_port: 10001,
            connection_idle_timeout: None,
        };
        let mut node1_p2p = P2PService::new(config1.clone());

        let config2 = Config {
            keypair: Keypair::generate_ed25519(),
            network_name: "testnet".to_owned(),
            tcp_port: 10002,
            connection_idle_timeout: None,
        };
        let mut node2_p2p = P2PService::new(config2);

        node1_p2p.start().await;
        node2_p2p.start().await;
    }
}
