use libp2p::gossipsub::{Sha256Topic, TopicHash};

pub const CONSENSUS_TOPIC: &str = "consensus";
pub const BLOCK_TOPIC: &str = "block";

#[derive(Clone, Debug)]
pub enum GossipTopicTag {
    Consensus,
    Block,
}

#[derive(Clone, Debug)]
pub struct GossipTopics {
    consensus_topic: (TopicHash, Sha256Topic),
    block_topic: (TopicHash, Sha256Topic),
}

impl GossipTopics {
    pub fn new(network_name: &str) -> Self {
        assert!(!network_name.is_empty(), "Received empty network name");

        let consensus_topic = Sha256Topic::new(format!("{CONSENSUS_TOPIC}/{network_name}"));
        let block_topic = Sha256Topic::new(format!("{BLOCK_TOPIC}/{network_name}"));
        Self {
            consensus_topic: (consensus_topic.hash(), consensus_topic),
            block_topic: (block_topic.hash(), block_topic),
        }
    }

    pub fn get_gossip_tag(&self, topic_hash: &TopicHash) -> Option<GossipTopicTag> {
        match topic_hash {
            hash if hash == &self.consensus_topic.0 => Some(GossipTopicTag::Consensus),
            hash if hash == &self.block_topic.0 => Some(GossipTopicTag::Block),
            _ => None,
        }
    }

    pub fn get_topic_hash(&self, tag: GossipTopicTag) -> TopicHash {
        match tag {
            GossipTopicTag::Consensus => self.consensus_topic.0.clone(),
            GossipTopicTag::Block => self.block_topic.0.clone(),
        }
    }
}
