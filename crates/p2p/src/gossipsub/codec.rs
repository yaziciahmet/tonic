use anyhow::anyhow;

use super::{GossipMessage, GossipTopicTag};

pub struct GossipCodec {
    max_message_size: usize,
}

impl GossipCodec {
    pub fn new(max_message_size: usize) -> Self {
        Self { max_message_size }
    }

    pub fn serialize(&self, message: &GossipMessage) -> anyhow::Result<Vec<u8>> {
        let data = match message {
            GossipMessage::Dummy(v) => borsh::to_vec(v),
            GossipMessage::Consensus(msg) => borsh::to_vec(msg),
            GossipMessage::Block(block) => borsh::to_vec(block),
        }
        .map_err(|e| anyhow!("{e}"))?;

        self.verify_message_size(data.len())?;

        Ok(data)
    }

    pub fn deserialize(&self, tag: &GossipTopicTag, data: &[u8]) -> anyhow::Result<GossipMessage> {
        self.verify_message_size(data.len())?;

        match tag {
            GossipTopicTag::Dummy => Ok(GossipMessage::Dummy(
                borsh::from_slice(data).map_err(|e| anyhow!("{e}"))?,
            )),
            GossipTopicTag::Consensus => Ok(GossipMessage::Consensus(
                borsh::from_slice(data).map_err(|e| anyhow!("{e}"))?,
            )),
            GossipTopicTag::Block => Ok(GossipMessage::Block(
                borsh::from_slice(data).map_err(|e| anyhow!("{e}"))?,
            )),
        }
    }

    fn verify_message_size(&self, size: usize) -> anyhow::Result<()> {
        if size > self.max_message_size {
            Err(anyhow!(
                "P2P message size exceeds maximum limit. size={} limit={}",
                size,
                self.max_message_size
            ))
        } else {
            Ok(())
        }
    }
}
