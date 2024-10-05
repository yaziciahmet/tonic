use libp2p::{noise, tcp, yamux, PeerId, Swarm, SwarmBuilder};

use crate::behaviour::TonicBehaviour;
use crate::config::Config;

pub struct P2PService {
    /// Local peer id
    pub local_peer_id: PeerId,

    swarm: Swarm<TonicBehaviour>,
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
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use libp2p::identity::Keypair;

    use crate::config::Config;

    use super::P2PService;

    #[test]
    pub fn test_init_p2p_service() {
        let p2p_config = Config {
            keypair: Keypair::generate_ed25519(),
            network_name: "testet".to_owned(),
            tcp_port: 8888,
            connection_idle_timeout: Some(Duration::from_secs(30)),
        };
        P2PService::new(p2p_config);
    }
}
