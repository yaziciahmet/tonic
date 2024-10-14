mod behaviour;
mod config;
mod gossipsub;
mod identify;
mod kademlia;
mod p2p_service;
mod p2p_service_proxy;

pub use config::Config;
use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};
pub use p2p_service::{P2PService, TonicP2PEvent};
pub use p2p_service_proxy::P2PServiceProxy;
use tokio::sync::mpsc;

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

pub fn new_service_with_proxy(config: Config) -> (P2PService, P2PServiceProxy) {
    let (publish_message_tx, publish_message_rx) = bmrng::channel(16);
    let publish_message_rx = bmrng::RequestReceiverStream::new(publish_message_rx);

    let (new_p2p_event_tx, new_p2p_event_rx) = mpsc::channel(16);

    let p2p_service = P2PService::new(config, publish_message_rx, new_p2p_event_tx);

    let p2p_service_proxy = P2PServiceProxy::new(publish_message_tx);
    p2p_service_proxy.run_p2p_event_handler(new_p2p_event_rx);

    (p2p_service, p2p_service_proxy)
}

// #[cfg(test)]
// mod tests {
//     use libp2p::identity::Keypair;

//     use crate::config::Config;

//     use super::P2PService;

//     #[tokio::test]
//     pub async fn test_start_p2p_service() {
//         tonic::initialize_tracing(tracing::Level::TRACE);
//         let config1 = Config {
//             keypair: Keypair::generate_ed25519(),
//             network_name: "testnet".to_owned(),
//             tcp_port: 10001,
//             connection_idle_timeout: None,
//             bootstrap_nodes: vec![],
//         };
//         let mut node1_p2p = P2PService::new(config1.clone());

//         node1_p2p.start().await;

//         let node1_multiaddr = format!(
//             "/ip4/127.0.0.1/tcp/{}/p2p/{}",
//             config1.tcp_port, node1_p2p.local_peer_id
//         )
//         .parse()
//         .unwrap();

//         let config2 = Config {
//             keypair: Keypair::generate_ed25519(),
//             network_name: "testnet".to_owned(),
//             tcp_port: 10002,
//             connection_idle_timeout: None,
//             bootstrap_nodes: vec![node1_multiaddr],
//         };
//         let mut node2_p2p = P2PService::new(config2);

//         node2_p2p.start().await;
//     }
// }
