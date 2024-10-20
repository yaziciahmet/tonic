use std::time::Duration;

use libp2p::identity::Keypair;
use libp2p::Multiaddr;

#[derive(Clone, Debug)]
pub struct Config {
    /// The keypair used for handshake during communication with other p2p nodes.
    pub keypair: Keypair,

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
