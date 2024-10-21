mod codec;
mod messages;
mod topics;

use std::time::Duration;

pub use codec::*;
use libp2p::gossipsub::{self, MessageAuthenticity, MessageId, Sha256Topic};
pub use messages::*;
use sha2::{Digest, Sha256};
pub use topics::*;

use crate::config::Config;

pub fn build_gossipsub_behaviour(p2p_config: &Config) -> gossipsub::Behaviour {
    let mut gossipsub = gossipsub::Behaviour::new(
        MessageAuthenticity::Signed(p2p_config.keypair.clone()),
        default_gossipsub_config(),
    )
    .expect("Gossipsub behaviour should be initialized");

    let topics = vec![DUMMY_TOPIC];
    // Subscribe to gossip topics
    for topic in topics {
        let t = Sha256Topic::new(format!("{}/{}", topic, p2p_config.network_name));
        gossipsub
            .subscribe(&t)
            .unwrap_or_else(|_| panic!("Subscription to topic {topic} should be successful"));
    }

    gossipsub
}

fn default_gossipsub_config() -> gossipsub::Config {
    // Message id function which will prevent sending the same message twice
    let message_id_fn =
        |message: &gossipsub::Message| MessageId::from(&Sha256::digest(&message.data)[..]);

    gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(gossipsub::ValidationMode::Strict) // enforces message signing
        .message_id_fn(message_id_fn)
        .build()
        .expect("Gossipsub config should be built")
}
