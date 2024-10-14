use std::io;

#[derive(Clone, Debug)]
pub enum GossipTopicTag {
    Dummy,
}

#[derive(Clone, Debug)]
pub enum GossipMessage {
    Dummy(u64),
}

impl GossipMessage {
    pub fn tag(&self) -> GossipTopicTag {
        match self {
            GossipMessage::Dummy(_) => GossipTopicTag::Dummy,
        }
    }

    // TODO: consider something other than json serialization, also maybe make this trait?
    pub fn serialize(&self) -> Result<Vec<u8>, io::Error> {
        match self {
            GossipMessage::Dummy(v) => Ok(serde_json::to_vec(v)?),
        }
    }

    pub fn deserialize(tag: &GossipTopicTag, data: &[u8]) -> Result<Self, io::Error> {
        match tag {
            GossipTopicTag::Dummy => Ok(GossipMessage::Dummy(serde_json::from_slice(data)?)),
        }
    }
}
