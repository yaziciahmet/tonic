use std::time::Duration;

use libp2p::futures::StreamExt;
use libp2p::swarm::SwarmEvent;
use libp2p::{noise, tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder};

use crate::behaviour::TonicBehaviour;
use crate::config::Config;

pub struct P2PService {
    /// Local peer id
    pub local_peer_id: PeerId,

    swarm: Swarm<TonicBehaviour>,
    tcp_port: u16,
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

        Self {
            local_peer_id,
            swarm,
            tcp_port: config.tcp_port,
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
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use libp2p::identity::Keypair;

    use crate::config::Config;

    use super::P2PService;

    #[tokio::test]
    pub async fn test_start_p2p_service() {
        let p2p_config = Config {
            keypair: Keypair::generate_ed25519(),
            network_name: "testnet".to_owned(),
            tcp_port: 0,
            connection_idle_timeout: Some(Duration::from_secs(30)),
        };

        let mut p2p_service = P2PService::new(p2p_config);

        p2p_service.start().await;
    }
}
