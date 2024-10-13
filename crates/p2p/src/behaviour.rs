use libp2p::gossipsub::{MessageId, PublishError, TopicHash};
use libp2p::swarm::NetworkBehaviour;
use libp2p::{gossipsub, identify, kad};

use crate::config::Config;
use crate::gossipsub::build_gossipsub_behaviour;
use crate::identify::build_identify_behaviour;
use crate::kademlia::build_kademlia_behaviour;

#[derive(NetworkBehaviour)]
pub struct TonicBehaviour {
    /// Message propagation behaviour Gossipsub
    pub(crate) gossipsub: gossipsub::Behaviour,
    /// Discovery behaviour Kademlia
    pub(crate) kademlia: kad::Behaviour<kad::store::MemoryStore>,
    /// Identify behaviour
    pub(crate) identify: identify::Behaviour,
}

impl TonicBehaviour {
    pub fn new(p2p_config: &Config) -> Self {
        let gossipsub = build_gossipsub_behaviour(p2p_config);
        let kademlia = build_kademlia_behaviour(p2p_config);
        let identify = build_identify_behaviour(p2p_config);

        Self {
            gossipsub,
            kademlia,
            identify,
        }
    }

    pub fn publish_message(
        &mut self,
        topic_hash: TopicHash,
        encoded_data: Vec<u8>,
    ) -> Result<MessageId, PublishError> {
        self.gossipsub.publish(topic_hash, encoded_data)
    }
}
