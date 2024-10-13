use libp2p::kad::store::MemoryStore;
use libp2p::kad::{self, Mode};
use libp2p::StreamProtocol;

use crate::config::Config;
use crate::TryPeerId;

pub fn build_kademlia_behaviour(p2p_config: &Config) -> kad::Behaviour<MemoryStore> {
    let local_peer_id = p2p_config.keypair.public().to_peer_id();

    let memory_store = MemoryStore::new(local_peer_id);

    let mut kademlia = kad::Behaviour::with_config(
        local_peer_id,
        memory_store,
        default_kademlia_config(&p2p_config.network_name),
    );
    kademlia.set_mode(Some(Mode::Server));

    // Add bootstrap nodes
    for addr in &p2p_config.bootstrap_nodes {
        if let Some(peer_id) = addr.try_to_peer_id() {
            kademlia.add_address(&peer_id, addr.clone());
        } else {
            tracing::warn!("Bootstrap node address does not have peer id: {}", addr);
        }
    }

    if let Err(e) = kademlia.bootstrap() {
        tracing::warn!("Failed to bootstrap kademlia: {}", e);
    }

    kademlia
}

fn default_kademlia_config(network_name: &str) -> kad::Config {
    let protocol_name = format!("/tonic/kad/{network_name}/kad/1.0.0");
    kad::Config::new(
        StreamProtocol::try_from_owned(protocol_name).expect("Valid kademlia protocol name"),
    )
}
