use tonic_consensus::types::{FinalizedBlock, IBFTMessage};

#[derive(Clone, Debug)]
pub enum GossipTopicTag {
    Dummy,
    Consensus,
    Block,
}

#[derive(Debug)]
pub enum GossipMessage {
    Dummy(u64),
    Consensus(IBFTMessage),
    Block(FinalizedBlock),
}

impl GossipMessage {
    pub fn tag(&self) -> GossipTopicTag {
        match self {
            GossipMessage::Dummy(_) => GossipTopicTag::Dummy,
            GossipMessage::Consensus(_) => GossipTopicTag::Consensus,
            GossipMessage::Block(_) => GossipTopicTag::Block,
        }
    }
}
