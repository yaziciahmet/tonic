use anyhow::anyhow;
use tonic_codec::SizeBoundedCodec;

use super::{GossipMessage, GossipTopicTag};

pub struct GossipCodec {
    inner: SizeBoundedCodec,
}

impl GossipCodec {
    pub fn new(max_message_size: usize) -> Self {
        let inner = SizeBoundedCodec::new(max_message_size);
        Self { inner }
    }

    pub fn serialize(&self, message: &GossipMessage) -> anyhow::Result<Vec<u8>> {
        match message {
            GossipMessage::Dummy(v) => Ok(self.inner.serialize(v).map_err(|e| anyhow!("{}", e))?),
        }
    }

    pub fn deserialize(&self, tag: &GossipTopicTag, data: &[u8]) -> anyhow::Result<GossipMessage> {
        match tag {
            GossipTopicTag::Dummy => Ok(GossipMessage::Dummy(
                self.inner.deserialize(data).map_err(|e| anyhow!("{}", e))?,
            )),
        }
    }
}
