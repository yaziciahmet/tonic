use libp2p::identify;
use libp2p::identity::PublicKey;

use crate::config::Config;

pub fn build_identify_behaviour(p2p_config: &Config) -> identify::Behaviour {
    identify::Behaviour::new(default_identify_config(p2p_config.keypair.public()))
}

fn default_identify_config(local_public_key: PublicKey) -> identify::Config {
    identify::Config::new("/tonic/1.0".to_string(), local_public_key)
}