use std::time::Duration;

use libp2p::gossipsub::{self, MessageAuthenticity, MessageId};
use sha2::{Digest, Sha256};

use crate::config::Config;

pub fn build_gossipsub_behaviour(p2p_config: &Config) -> gossipsub::Behaviour {
    gossipsub::Behaviour::new(
        MessageAuthenticity::Signed(p2p_config.keypair.clone()),
        default_gossipsub_config(),
    )
    .expect("gossipsub::Behaviour to be initialized")
}

fn default_gossipsub_config() -> gossipsub::Config {
    // Message id function which will prevent sending the same message twice
    let message_id_fn =
        |message: &gossipsub::Message| MessageId::from(&Sha256::digest(&message.data)[..]);

    gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .protocol_id_prefix("/meshsub/1.0.0")
        .validation_mode(gossipsub::ValidationMode::Strict) // enforces message signing
        .message_id_fn(message_id_fn)
        .build()
        .expect("gossipsub::Config to be built")
}
