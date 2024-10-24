mod behaviour;
pub mod config;
mod gossipsub;
mod identify;
mod kademlia;
pub mod p2p_proxy;
pub mod p2p_service;

pub use config::*;
pub use gossipsub::GossipMessage;
use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};
pub use p2p_proxy::*;
pub use p2p_service::*;

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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use libp2p::{identity::Keypair, Multiaddr, PeerId};

    use super::{build_proxy, Config, GossipMessage, P2PService, P2PServiceProxy};

    async fn initialize_node(
        tcp_port: u16,
        bootstrap_nodes: Vec<Multiaddr>,
    ) -> (P2PServiceProxy, PeerId, Multiaddr) {
        let key = Keypair::generate_ed25519();
        let config = Config {
            keypair: key.clone(),
            network_name: "testnet".to_owned(),
            tcp_port,
            connection_idle_timeout: None,
            bootstrap_nodes,
        };

        let (p2p_proxy, request_receiver) = build_proxy();
        let mut p2p = P2PService::new(config, request_receiver, p2p_proxy.clone());
        p2p.listen().await;

        let peer_id = p2p.local_peer_id.clone();
        let addr = p2p.address.clone().unwrap();

        // Spawn the runner in the background
        tokio::spawn(async move {
            p2p.run().await;
        });

        // Sleep some time to ensure that p2p setup is complete
        tokio::time::sleep(Duration::from_millis(50)).await;

        (p2p_proxy, peer_id, addr)
    }

    #[tokio::test]
    async fn test_p2p_initialize() {
        let (_, peer_id, addr) = initialize_node(0, vec![]).await;
        initialize_node(0, vec![addr.with_p2p(peer_id).unwrap()]).await;
    }

    #[tokio::test]
    async fn test_gossipsub() {
        let (node1_proxy, node1_peer_id, node1_addr) = initialize_node(0, vec![]).await;
        let (node2_proxy, node2_peer_id, _) =
            initialize_node(0, vec![node1_addr.with_p2p(node1_peer_id).unwrap()]).await;

        let mut node1_rx = node1_proxy.subscribe_dummy();
        let mut node2_rx = node2_proxy.subscribe_dummy();

        // Node1 broadcasts message
        let value = 69;
        node1_proxy
            .broadcast_message(GossipMessage::Dummy(value))
            .unwrap();

        // Node2 should receive from Node1
        let (received_value, received_peer_id) =
            tokio::time::timeout(Duration::from_secs(1), node2_rx.recv())
                .await
                .unwrap()
                .unwrap();

        assert_eq!(value, received_value);
        assert_eq!(node1_peer_id, received_peer_id);

        // Ensure that Node1 didn't receive its own message
        assert!(node1_rx.try_recv().is_err());

        // Node2 broadcasts message
        let value = 42;
        node2_proxy
            .broadcast_message(GossipMessage::Dummy(value))
            .unwrap();

        //  Node1 should receive from Node2
        let (received_value, received_peer_id) =
            tokio::time::timeout(Duration::from_secs(1), node1_rx.recv())
                .await
                .unwrap()
                .unwrap();

        assert_eq!(value, received_value);
        assert_eq!(node2_peer_id, received_peer_id);

        // Ensure that Node2 didn't receive its own message
        assert!(node2_rx.try_recv().is_err());
    }
}
