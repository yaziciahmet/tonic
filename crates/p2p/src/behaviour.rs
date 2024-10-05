use libp2p::{kad, gossipsub, identify};
use libp2p::swarm::NetworkBehaviour;

use crate::config::Config;
use crate::kademlia::build_kademlia_behaviour;
use crate::gossipsub::build_gossipsub_behaviour;
use crate::identify::build_identify_behaviour;

#[derive(NetworkBehaviour)]
pub struct TonicBehaviour {
    /// Message propagation behaviour Gossipsub
    gossipsub: gossipsub::Behaviour,
    /// Discovery behaviour Kademlia
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    /// Identify behaviour
    identify: identify::Behaviour,
}

impl TonicBehaviour {
    pub fn new(p2p_config: &Config) -> Self {
        let gossipsub = build_gossipsub_behaviour(p2p_config);
        let kademlia = build_kademlia_behaviour(p2p_config);
        let identify = build_identify_behaviour(p2p_config);

        Self { gossipsub, kademlia, identify }
    }
}
