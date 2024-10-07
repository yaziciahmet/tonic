use std::io;

use libp2p::gossipsub::TopicHash;

const DUMMY_TOPIC: &str = "dummy";

#[derive(Clone, Debug)]
pub enum GossipsubTopic {
    Dummy,
}

impl From<GossipsubTopic> for String {
    fn from(topic: GossipsubTopic) -> Self {
        match topic {
            GossipsubTopic::Dummy => DUMMY_TOPIC.to_owned(),
        }
    }
}

// TODO: Actually hash the topic and calculate at the initialization once
impl From<GossipsubTopic> for TopicHash {
    fn from(topic: GossipsubTopic) -> Self {
        TopicHash::from_raw(topic)
    }
}

impl GossipsubTopic {
    pub fn from_topic_hash(hash: TopicHash) -> Option<Self> {
        match hash.as_str() {
            DUMMY_TOPIC => Some(GossipsubTopic::Dummy),
            _ => None,
        }
    }
}

pub enum GossipsubMessage {
    Dummy(u64),
}

impl GossipsubMessage {
    pub fn topic(&self) -> GossipsubTopic {
        match self {
            GossipsubMessage::Dummy(_) => GossipsubTopic::Dummy,
        }
    }

    // TODO: consider something other than json serialization, also maybe make this trait?
    pub fn serialize(&self) -> Result<Vec<u8>, io::Error> {
        match self {
            GossipsubMessage::Dummy(v) => Ok(serde_json::to_vec(v)?),
        }
    }

    pub fn deserialize(topic: &GossipsubTopic, data: &[u8]) -> Result<Self, io::Error> {
        match topic {
            GossipsubTopic::Dummy => Ok(GossipsubMessage::Dummy(serde_json::from_slice(data)?)),
        }
    }
}
