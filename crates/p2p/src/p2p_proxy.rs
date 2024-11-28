use tokio::sync::{broadcast, mpsc};

use crate::gossipsub::GossipMessage;
use crate::p2p_service::{Broadcast, IncomingDummyMessage, P2PRequest};
use crate::IncomingConsensusMessage;

const CHANNEL_SIZE: usize = 1024;

#[derive(Clone)]
pub struct P2PServiceProxy {
    request_sender: mpsc::Sender<P2PRequest>,
    dummy_broadcast: broadcast::Sender<IncomingDummyMessage>,
    consensus_sender: mpsc::Sender<IncomingConsensusMessage>,
}

impl P2PServiceProxy {
    pub fn new(
        request_sender: mpsc::Sender<P2PRequest>,
        consensus_sender: mpsc::Sender<IncomingConsensusMessage>,
    ) -> Self {
        let (dummy_broadcast, _) = broadcast::channel(CHANNEL_SIZE);
        Self {
            request_sender,
            dummy_broadcast,
            consensus_sender,
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

    fn broadcast_consensus(&self, data: IncomingConsensusMessage) -> anyhow::Result<()> {
        self.consensus_sender.blocking_send(data)?;
        Ok(())
    }
}

/// Builds proxy with a sender channel.
/// Returns proxy and the receiver channel.
pub fn build_proxy() -> (
    P2PServiceProxy,
    mpsc::Receiver<P2PRequest>,
    mpsc::Receiver<IncomingConsensusMessage>,
) {
    let (request_sender, request_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (consensus_sender, consensus_receiver) = mpsc::channel(CHANNEL_SIZE);
    (
        P2PServiceProxy::new(request_sender, consensus_sender),
        request_receiver,
        consensus_receiver,
    )
}
