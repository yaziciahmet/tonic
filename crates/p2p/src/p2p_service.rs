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
        let local_peer_id = config.keypair.public().to_peer_id();
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

        Self { local_peer_id, swarm }
    }
}
