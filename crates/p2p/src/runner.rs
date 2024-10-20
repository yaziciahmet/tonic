use libp2p::{Multiaddr, PeerId};
use tokio::select;
use tokio::sync::{broadcast, mpsc};
use tracing::error;

use crate::config::Config;
use crate::gossipsub::GossipMessage;
use crate::p2p_service::{P2PService, TonicP2PEvent};

const CHANNEL_SIZE: usize = 1024;

#[derive(Debug)]
pub enum P2PRequest {
    BroadcastMessage(GossipMessage),
}

#[derive(Clone, Debug)]
pub struct IncomingDummyMessage(pub PeerId, pub u64);

pub trait Broadcast {
    fn broadcast_dummy(&self, data: IncomingDummyMessage) -> anyhow::Result<()>;
}

#[derive(Clone)]
pub struct P2PServiceProxy {
    request_sender: mpsc::Sender<P2PRequest>,
    dummy_broadcast: broadcast::Sender<IncomingDummyMessage>,
}

impl P2PServiceProxy {
    pub fn new(request_sender: mpsc::Sender<P2PRequest>) -> Self {
        let (dummy_broadcast, _) = broadcast::channel(CHANNEL_SIZE);

        Self {
            request_sender,
            dummy_broadcast,
        }
    }

    pub fn broadcast_message(&self, message: GossipMessage) -> anyhow::Result<()> {
        let request = P2PRequest::BroadcastMessage(message);
        self.request_sender.try_send(request)?;
        Ok(())
    }

    pub fn subscribe_dummy(&self) -> broadcast::Receiver<IncomingDummyMessage> {
        self.dummy_broadcast.subscribe()
    }
}

impl Broadcast for P2PServiceProxy {
    fn broadcast_dummy(&self, data: IncomingDummyMessage) -> anyhow::Result<()> {
        self.dummy_broadcast.send(data)?;
        Ok(())
    }
}

pub struct P2PRunner<B>
where
    B: Broadcast,
{
    p2p_service: P2PService,
    broadcast: B,
    request_receiver: mpsc::Receiver<P2PRequest>,
}

impl<B> P2PRunner<B>
where
    B: Broadcast,
{
    pub async fn new(
        config: Config,
        broadcast: B,
        request_receiver: mpsc::Receiver<P2PRequest>,
    ) -> Self {
        let mut p2p_service = P2PService::new(config);
        p2p_service.start().await;

        Self {
            p2p_service,
            broadcast,
            request_receiver,
        }
    }

    pub async fn run(&mut self) {
        loop {
            select! {
                Some(event) = self.p2p_service.next_event() => {
                    if let Err(err) = self.handle_p2p_event(event) {
                        error!("Failed to handle P2P event: {}", err);
                    }
                }
                request = self.request_receiver.recv() => {
                    let request = request.expect("P2P request should never be empty");
                    if let Err(err) = self.handle_request(request) {
                        error!("Failed to handle P2P request: {}", err);
                    }
                }
            }
        }
    }

    fn handle_p2p_event(&self, event: TonicP2PEvent) -> anyhow::Result<()> {
        match event {
            TonicP2PEvent::GossipsubMessage {
                peer_id, message, ..
            } => match message {
                GossipMessage::Dummy(value) => self
                    .broadcast
                    .broadcast_dummy(IncomingDummyMessage(peer_id, value)),
            },
        }
    }

    fn handle_request(&mut self, request: P2PRequest) -> anyhow::Result<()> {
        match request {
            P2PRequest::BroadcastMessage(message) => self.p2p_service.publish_message(message)?,
        };

        Ok(())
    }

    #[cfg(feature = "test-helpers")]
    pub fn multiaddr(&self) -> Option<Multiaddr> {
        self.p2p_service.address.clone()
    }
}

/// Builds proxy with a sender channel.
/// Returns proxy and the receiver channel.
pub fn build_proxy() -> (P2PServiceProxy, mpsc::Receiver<P2PRequest>) {
    let (request_sender, request_receiver) = mpsc::channel(CHANNEL_SIZE);
    (P2PServiceProxy::new(request_sender), request_receiver)
}

#[cfg(test)]
#[cfg(feature = "test-helpers")]
mod tests {
    use std::time::Duration;

    use libp2p::{identity::Keypair, Multiaddr, PeerId};

    use crate::config::Config;
    use crate::gossipsub::GossipMessage;
    use crate::{IncomingDummyMessage, P2PServiceProxy};

    use super::{build_proxy, P2PRunner};

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

        let (proxy, request_receiver) = build_proxy();
        let mut runner = P2PRunner::new(config, proxy.clone(), request_receiver).await;

        let addr = runner.multiaddr().unwrap();

        // Spawn the runner in the background
        tokio::spawn(async move {
            runner.run().await;
        });

        // Sleep some time to ensure that p2p setup is complete
        tokio::time::sleep(Duration::from_secs(2)).await;

        (proxy, key.public().to_peer_id(), addr)
    }

    #[tokio::test]
    async fn test_p2p_initialize() {
        let (_, peer_id, addr) = initialize_node(0, vec![]).await;
        initialize_node(0, vec![addr.with_p2p(peer_id).unwrap()]).await;
    }

    #[tokio::test]
    async fn test_gossipsub() {
        tonic::initialize_tracing(tracing::Level::TRACE);
        let (node1_proxy, node1_peer_id, node1_addr) = initialize_node(0, vec![]).await;
        let (node2_proxy, node2_peer_id, _) =
            initialize_node(0, vec![node1_addr.with_p2p(node1_peer_id).unwrap()]).await;

        let mut node2_rx = node2_proxy.subscribe_dummy();

        let value = 69;
        node1_proxy
            .broadcast_message(GossipMessage::Dummy(value))
            .unwrap();

        let IncomingDummyMessage(received_peer_id, received_value) =
            tokio::time::timeout(Duration::from_secs(2), node2_rx.recv())
                .await
                .unwrap()
                .unwrap();

        assert_eq!(value, received_value);
        assert_eq!(node1_peer_id, received_peer_id);

        let mut node1_rx = node1_proxy.subscribe_dummy();

        let value = 42;
        node2_proxy
            .broadcast_message(GossipMessage::Dummy(value))
            .unwrap();

        let IncomingDummyMessage(received_peer_id, received_value) =
            tokio::time::timeout(Duration::from_secs(2), node1_rx.recv())
                .await
                .unwrap()
                .unwrap();

        assert_eq!(value, received_value);
        assert_eq!(node2_peer_id, received_peer_id);
    }
}
