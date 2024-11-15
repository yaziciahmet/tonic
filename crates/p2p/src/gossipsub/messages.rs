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
}
