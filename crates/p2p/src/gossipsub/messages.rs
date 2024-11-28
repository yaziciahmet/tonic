use tonic_consensus_core::types::IBFTMessage;

#[derive(Clone, Debug)]
pub enum GossipTopicTag {
    Dummy,
    Consensus,
}

#[derive(Debug)]
pub enum GossipMessage {
    Dummy(u64),
    Consensus(IBFTMessage),
}

impl GossipMessage {
    pub fn tag(&self) -> GossipTopicTag {
        match self {
            GossipMessage::Dummy(_) => GossipTopicTag::Dummy,
            GossipMessage::Consensus(_) => GossipTopicTag::Consensus,
        }
    }
}
