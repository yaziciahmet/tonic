use std::time::Duration;

use libp2p::identity::{secp256k1, Keypair};
use libp2p::Multiaddr;

#[derive(Clone, Debug)]
pub struct Config {
    /// The keypair used for handshake during communication with other p2p nodes.
    pub keypair: Keypair,

    /// Whether the current node is validator. Used for determining the
    /// topics to subscribe, such as consensus topic.
    pub is_validator: bool,

    /// Name of the Network
    pub network_name: String,

    /// The TCP port that Swarm listens on
    pub tcp_port: u16,

    // `DiscoveryBehaviour` related fields
    /// Multiaddresses of discovery initiation nodes
    pub bootstrap_nodes: Vec<Multiaddr>,
    /// Connection timeout duration on idle connections
    pub connection_idle_timeout: Option<Duration>,
}

/// Takes secret key bytes generated outside of libp2p.
/// And converts it into libp2p's `Keypair::Secp256k1`.
pub fn convert_to_libp2p_keypair(secret_key_bytes: impl AsMut<[u8]>) -> anyhow::Result<Keypair> {
    let secret_key = secp256k1::SecretKey::try_from_bytes(secret_key_bytes)?;
    let keypair: secp256k1::Keypair = secret_key.into();

    Ok(keypair.into())
}
