use tokio::sync::{broadcast, mpsc};
use tonic_consensus::types::IBFTMessage;

use crate::gossipsub::GossipMessage;
use crate::p2p_service::{Broadcast, IncomingDummyMessage, P2PRequest};

const CHANNEL_SIZE: usize = 1024;

#[derive(Clone)]
pub struct P2PServiceProxy {
    request_sender: mpsc::Sender<P2PRequest>,
    dummy_tx: broadcast::Sender<IncomingDummyMessage>,
    // Consensus messages have only one receiver, hence mpsc instead of broadcast
    consensus_tx: mpsc::Sender<IBFTMessage>,
}

impl P2PServiceProxy {
    pub fn new(
        request_sender: mpsc::Sender<P2PRequest>,
        consensus_tx: mpsc::Sender<IBFTMessage>,
    ) -> Self {
        let (dummy_tx, _) = broadcast::channel(CHANNEL_SIZE);
        Self {
            request_sender,
            dummy_tx,
            consensus_tx,
        }
    }

    pub fn broadcast_message(&self, message: GossipMessage) -> anyhow::Result<()> {
        let request = P2PRequest::BroadcastMessage(message);
        self.request_sender.try_send(request)?;
        Ok(())
    }

    pub fn subscribe_dummy(&self) -> broadcast::Receiver<IncomingDummyMessage> {
        self.dummy_tx.subscribe()
    }
}

impl Broadcast for P2PServiceProxy {
    fn broadcast_dummy(&self, data: IncomingDummyMessage) -> anyhow::Result<()> {
        self.dummy_tx.send(data)?;
        Ok(())
    }

    fn broadcast_consensus(&self, data: IBFTMessage) -> anyhow::Result<()> {
        self.consensus_tx.blocking_send(data)?;
        Ok(())
    }
}

/// Builds proxy with a sender channel.
/// Returns proxy and the receiver channel.
pub fn build_proxy() -> (
    P2PServiceProxy,
    mpsc::Receiver<P2PRequest>,
    mpsc::Receiver<IBFTMessage>,
) {
    let (request_sender, request_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (consensus_tx, consensus_rx) = mpsc::channel(CHANNEL_SIZE);
    (
        P2PServiceProxy::new(request_sender, consensus_tx),
        request_receiver,
        consensus_rx,
    )
}
