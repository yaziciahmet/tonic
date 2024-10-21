use tokio::sync::{broadcast, mpsc};

use crate::gossipsub::GossipMessage;
use crate::p2p_service::{Broadcast, IncomingDummyMessage, P2PRequest};

const CHANNEL_SIZE: usize = 1024;

#[derive(Clone)]
pub struct P2PServiceProxy {
    request_sender: mpsc::Sender<P2PRequest>,
    dummy_broadcast: broadcast::Sender<IncomingDummyMessage>,
}

impl P2PServiceProxy {
    pub fn new(request_sender: mpsc::Sender<P2PRequest>) -> Self {
        assert!(
            !request_sender.is_closed(),
            "Received closed request sender channel"
        );

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

/// Builds proxy with a sender channel.
/// Returns proxy and the receiver channel.
pub fn build_proxy() -> (P2PServiceProxy, mpsc::Receiver<P2PRequest>) {
    let (request_sender, request_receiver) = mpsc::channel(CHANNEL_SIZE);
    (P2PServiceProxy::new(request_sender), request_receiver)
}
