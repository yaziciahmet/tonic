use libp2p::gossipsub::{Sha256Topic, TopicHash};

use super::messages::{GossipMessage, GossipTopicTag};

pub const DUMMY_TOPIC: &str = "dummy";
pub const CONSENSUS_TOPIC: &str = "consensus";

#[derive(Debug)]
pub struct GossipTopics {
    dummy_topic: (TopicHash, Sha256Topic),
    consensus_topic: (TopicHash, Sha256Topic),
}

impl GossipTopics {
    pub fn new(network_name: &str) -> Self {
        assert!(!network_name.is_empty(), "Received empty network name");

        let dummy_topic = Sha256Topic::new(format!("{DUMMY_TOPIC}/{network_name}"));
        let consensus_topic = Sha256Topic::new(format!("{CONSENSUS_TOPIC}/{network_name}"));
        Self {
            dummy_topic: (dummy_topic.hash(), dummy_topic),
            consensus_topic: (consensus_topic.hash(), consensus_topic),
        }
    }

    pub fn get_gossip_tag(&self, topic_hash: &TopicHash) -> Option<GossipTopicTag> {
        match topic_hash {
            hash if hash == &self.dummy_topic.0 => Some(GossipTopicTag::Dummy),
            hash if hash == &self.consensus_topic.0 => Some(GossipTopicTag::Consensus),
            _ => None,
        }
    }

    pub fn get_topic_hash_from_message(&self, message: &GossipMessage) -> TopicHash {
        match message {
            GossipMessage::Dummy(_) => self.dummy_topic.0.clone(),
            GossipMessage::Consensus(_) => self.consensus_topic.0.clone(),
        }
    }
}
