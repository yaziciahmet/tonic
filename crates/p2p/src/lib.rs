mod behaviour;
pub mod config;
pub mod gossipsub;
mod identify;
mod kademlia;
mod p2p_service;
pub mod runner;

pub use config::Config;
pub use gossipsub::GossipMessage;
use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};
pub use runner::{build_proxy, IncomingDummyMessage, P2PServiceProxy};

pub trait TryPeerId {
    /// Tries convert `Self` into `PeerId`.
    fn try_to_peer_id(&self) -> Option<PeerId>;
}

impl TryPeerId for Multiaddr {
    fn try_to_peer_id(&self) -> Option<PeerId> {
        self.iter().last().and_then(|p| match p {
            Protocol::P2p(peer_id) => Some(peer_id),
            _ => None,
        })
    }
}
