mod behaviour;
pub mod config;
mod gossipsub;
mod identify;
mod kademlia;
pub mod p2p_proxy;
pub mod p2p_service;

pub use config::*;
use libp2p::multiaddr::Protocol;
pub use libp2p::{Multiaddr, PeerId};
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

    use libp2p::identity::Keypair;
    use libp2p::{Multiaddr, PeerId};
    use tokio::sync::mpsc;
    use tonic_consensus_poa::backend::Broadcast;
    use tonic_consensus_poa::types::{FinalizedBlock, ProposedBlock};

    use crate::BuildProxyResult;

    use super::{build_proxy, Config, P2PService, P2PServiceProxy};

    async fn initialize_node(
        tcp_port: u16,
        bootstrap_nodes: Vec<Multiaddr>,
    ) -> (
        P2PServiceProxy,
        PeerId,
        Multiaddr,
        mpsc::Receiver<FinalizedBlock>,
    ) {
        let key = Keypair::generate_ed25519();
        let config = Config {
            keypair: key.clone(),
            is_validator: false,
            network_name: "testnet".to_owned(),
            tcp_port,
            connection_idle_timeout: None,
            bootstrap_nodes,
        };

        let BuildProxyResult {
            proxy,
            request_rx,
            block_rx,
            ..
        } = build_proxy(&config.network_name);
        let mut p2p = P2PService::new(config, request_rx, proxy.clone());
        p2p.listen().await;

        let peer_id = p2p.local_peer_id;
        let addr = p2p.address.clone().unwrap();

        // Spawn the runner in the background
        tokio::spawn(async move {
            p2p.run().await;
        });

        // Sleep some time to ensure that p2p setup is complete
        tokio::time::sleep(Duration::from_millis(50)).await;

        (proxy, peer_id, addr, block_rx)
    }

    #[tokio::test]
    async fn p2p_initialize() {
        let (_, peer_id, addr, _) = initialize_node(0, vec![]).await;
        initialize_node(0, vec![addr.with_p2p(peer_id).unwrap()]).await;
    }

    #[tokio::test]
    async fn gossipsub_messaging() {
        let (node1_proxy, node1_peer_id, node1_addr, mut node1_rx) =
            initialize_node(0, vec![]).await;
        let (node2_proxy, _, _, mut node2_rx) =
            initialize_node(0, vec![node1_addr.with_p2p(node1_peer_id).unwrap()]).await;

        // Node1 broadcasts message
        let block = FinalizedBlock::new(ProposedBlock::new(vec![1, 2, 3], 0), vec![]);
        node1_proxy.broadcast_block(&block).await;

        // Node2 should receive from Node1
        let received_block = tokio::time::timeout(Duration::from_secs(1), node2_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(received_block.raw_block(), vec![1, 2, 3]);
        assert_eq!(received_block.proof().round(), 0);

        // Ensure that Node1 didn't receive its own message
        assert!(node1_rx.try_recv().is_err());

        // Node2 broadcasts message
        let block = FinalizedBlock::new(ProposedBlock::new(vec![4, 5, 6], 0), vec![]);
        node2_proxy.broadcast_block(&block).await;

        //  Node1 should receive from Node2
        let received_block = tokio::time::timeout(Duration::from_secs(1), node1_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(received_block.raw_block(), vec![4, 5, 6]);
        assert_eq!(received_block.proof().round(), 0);

        // Ensure that Node2 didn't receive its own message
        assert!(node2_rx.try_recv().is_err());
    }
}
