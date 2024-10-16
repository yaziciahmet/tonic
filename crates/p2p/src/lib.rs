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
use tokio::sync::{mpsc, oneshot};

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

/// Starts the [`P2PService`] at the background to handle both incoming and outgoing requests
/// and returns a [`P2PServiceProxy`] to interact with [`P2PService`] in a thread-safe manner.
pub async fn start_p2p_service(config: Config) -> (P2PServiceProxy, PeerId, Multiaddr) {
    let (publish_message_tx, publish_message_rx) = bmrng::channel(16);
    let publish_message_rx = bmrng::RequestReceiverStream::new(publish_message_rx);

    let (new_p2p_event_tx, new_p2p_event_rx) = mpsc::channel(16);

    // Initialize p2p service
    let mut service = P2PService::new(config, publish_message_rx, new_p2p_event_tx);

    // Initialize p2p proxy
    let service_proxy = P2PServiceProxy::new(publish_message_tx);
    service_proxy.run_p2p_event_handler(new_p2p_event_rx);

    // Hand over the p2p service to run at background
    let (ready_tx, ready_rx) = oneshot::channel();
    tokio::spawn(async move {
        service.start(Some(ready_tx)).await;
    });

    // Wait for p2p service to be ready
    let (peer_id, multiaddr) = ready_rx.await.unwrap();

    (service_proxy, peer_id, multiaddr)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use libp2p::{identity::Keypair, Multiaddr, PeerId};

    use crate::{config::Config, gossipsub::GossipMessage, P2PServiceProxy};

    pub async fn initialize_node(
        tcp_port: u16,
        bootstrap_nodes: Vec<Multiaddr>,
    ) -> (P2PServiceProxy, PeerId, Multiaddr) {
        let config = Config {
            keypair: Keypair::generate_ed25519(),
            network_name: "testnet".to_owned(),
            tcp_port,
            connection_idle_timeout: None,
            bootstrap_nodes,
        };
        let result = super::start_p2p_service(config).await;

        // Sleep some time to ensure that p2p setup is complete
        tokio::time::sleep(Duration::from_secs(1)).await;

        result
    }

    #[tokio::test]
    pub async fn test_p2p_initialize() {
        let (_, peer_id, addr) = initialize_node(0, vec![]).await;
        initialize_node(0, vec![addr.with_p2p(peer_id).unwrap()]).await;
    }

    #[tokio::test]
    pub async fn test_gossipsub() {
        let (node1_proxy, node1_peer_id, node1_addr) = initialize_node(0, vec![]).await;
        let (node2_proxy, node2_peer_id, _) =
            initialize_node(0, vec![node1_addr.with_p2p(node1_peer_id).unwrap()]).await;

        let mut node2_rx = node2_proxy.subscribe_dummy_messages();

        let value = 69;
        node1_proxy
            .publish_message(GossipMessage::Dummy(value))
            .await
            .unwrap();

        let (received_peer_id, received_value) =
            tokio::time::timeout(Duration::from_secs(1), node2_rx.recv())
                .await
                .unwrap()
                .unwrap();

        assert_eq!(value, received_value);
        assert_eq!(node1_peer_id, received_peer_id);

        let mut node1_rx = node1_proxy.subscribe_dummy_messages();

        let value = 42;
        node2_proxy
            .publish_message(GossipMessage::Dummy(value))
            .await
            .unwrap();

        let (received_peer_id, received_value) =
            tokio::time::timeout(Duration::from_secs(1), node1_rx.recv())
                .await
                .unwrap()
                .unwrap();

        assert_eq!(value, received_value);
        assert_eq!(node2_peer_id, received_peer_id);
    }
}
