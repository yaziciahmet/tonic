use anyhow::anyhow;
use async_trait::async_trait;
use libp2p::gossipsub::TopicHash;
use tokio::sync::mpsc;
use tonic_consensus_poa::types::{FinalizedBlock, IBFTBroadcastMessage, IBFTReceivedMessage};
use tracing::warn;

use crate::gossipsub::{GossipCodec, GossipTopicTag, GossipTopics};
use crate::p2p_service::{self, P2PRequest};

const MAX_MESSAGE_SIZE: usize = 1024 * 1024 * 4; // 4 MB
const CHANNEL_SIZE: usize = 1024;

#[derive(Clone)]
pub struct P2PServiceProxy {
    codec: GossipCodec,
    topics: GossipTopics,

    request_sender: mpsc::Sender<P2PRequest>,
    // Consensus messages have only one receiver, hence mpsc instead of broadcast
    consensus_tx: mpsc::Sender<IBFTReceivedMessage>,
    block_tx: mpsc::Sender<FinalizedBlock>,
}

impl P2PServiceProxy {
    pub fn new(
        network_name: &str,
        request_sender: mpsc::Sender<P2PRequest>,
        consensus_tx: mpsc::Sender<IBFTReceivedMessage>,
        block_tx: mpsc::Sender<FinalizedBlock>,
    ) -> Self {
        let codec = GossipCodec::new(MAX_MESSAGE_SIZE);
        let topics = GossipTopics::new(network_name);
        Self {
            codec,
            topics,
            request_sender,
            consensus_tx,
            block_tx,
        }
    }

    async fn broadcast_network(&self, tag: GossipTopicTag, data: Vec<u8>) {
        let topic_hash = self.topics.get_topic_hash(tag);
        let request = P2PRequest::Broadcast(topic_hash, data);
        self.request_sender
            .send(request)
            .await
            .expect("Request receiver channel should never be closed");
    }

    fn relay_message_by_tag(&self, tag: GossipTopicTag, data: Vec<u8>) -> anyhow::Result<()> {
        match tag {
            GossipTopicTag::Consensus => {
                let ibft_message = self.codec.deserialize(&data)?;
                if let Err(err) = self.consensus_tx.try_send(ibft_message) {
                    match err {
                        mpsc::error::TrySendError::Closed(_) => {
                            panic!("Consensus message receiver should never be closed")
                        }
                        mpsc::error::TrySendError::Full(_) => {
                            warn!("Consensus message channel is full")
                        }
                    }
                }
            }
            GossipTopicTag::Block => {
                let block = self.codec.deserialize(&data)?;
                if let Err(err) = self.block_tx.try_send(block) {
                    match err {
                        mpsc::error::TrySendError::Closed(_) => {
                            panic!("Block message receiver should never be closed")
                        }
                        mpsc::error::TrySendError::Full(_) => {
                            warn!("Block message channel is full")
                        }
                    }
                }
            }
        };
        Ok(())
    }
}

impl p2p_service::Relayer for P2PServiceProxy {
    fn relay_message(&self, topic_hash: TopicHash, data: Vec<u8>) -> anyhow::Result<()> {
        let tag = self
            .topics
            .get_gossip_tag(&topic_hash)
            .ok_or(anyhow!("Invalid topic hash"))?;
        self.relay_message_by_tag(tag, data)
    }
}

#[async_trait]
impl tonic_consensus_poa::backend::Broadcast for P2PServiceProxy {
    async fn broadcast_message<'a>(&self, message: IBFTBroadcastMessage<'a>) {
        let data = self
            .codec
            .serialize(&message)
            .expect("IBFT message serialization should not fail");

        self.broadcast_network(GossipTopicTag::Consensus, data)
            .await;
    }

    async fn broadcast_block(&self, block: &FinalizedBlock) {
        let data = self
            .codec
            .serialize(block)
            .expect("Block serialization should not fail");

        self.broadcast_network(GossipTopicTag::Block, data).await;
    }
}

/// Builds proxy with a sender channel.
/// Returns proxy and the receiver channel.
pub fn build_proxy(network_name: &str) -> BuildProxyResult {
    let (request_sender, request_rx) = mpsc::channel(CHANNEL_SIZE);
    let (consensus_tx, consensus_rx) = mpsc::channel(CHANNEL_SIZE);
    let (block_tx, block_rx) = mpsc::channel(CHANNEL_SIZE);
    BuildProxyResult {
        proxy: P2PServiceProxy::new(network_name, request_sender, consensus_tx, block_tx),
        request_rx,
        consensus_rx,
        block_rx,
    }
}

pub struct BuildProxyResult {
    pub proxy: P2PServiceProxy,
    pub request_rx: mpsc::Receiver<P2PRequest>,
    pub consensus_rx: mpsc::Receiver<IBFTReceivedMessage>,
    pub block_rx: mpsc::Receiver<FinalizedBlock>,
}
