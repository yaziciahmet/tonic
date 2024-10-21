use anyhow::anyhow;
use bincode::Options;

use super::{GossipMessage, GossipTopicTag};

pub struct GossipCodec {
    max_message_size: u64,
}

impl GossipCodec {
    pub fn new(max_message_size: u64) -> Self {
        Self { max_message_size }
    }

    pub fn serialize(&self, message: &GossipMessage) -> anyhow::Result<Vec<u8>> {
        let opts = bincode::DefaultOptions::new()
            .with_varint_encoding()
            .with_little_endian()
            .with_limit(self.max_message_size);
        match message {
            GossipMessage::Dummy(v) => Ok(opts.serialize(v).map_err(|e| anyhow!("{:?}", e))?),
        }
    }

    pub fn deserialize(&self, tag: &GossipTopicTag, data: &[u8]) -> anyhow::Result<GossipMessage> {
        let opts = bincode::DefaultOptions::new()
            .with_varint_encoding()
            .with_little_endian()
            .with_limit(self.max_message_size);
        match tag {
            GossipTopicTag::Dummy => Ok(GossipMessage::Dummy(
                opts.deserialize(data).map_err(|e| anyhow!("{:?}", e))?,
            )),
        }
    }
}
