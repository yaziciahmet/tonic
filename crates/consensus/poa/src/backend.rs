use std::fmt::{Debug, Display};

use async_trait::async_trait;
use tonic_primitives::Address;

use crate::types::{FinalizedBlock, IBFTBroadcastMessage, View};

pub trait ValidatorManager: Clone + Send + Sync + 'static {
    fn is_validator(&self, address: Address, height: u64) -> bool;

    fn is_proposer(&self, address: Address, view: View) -> bool;

    fn quorum(&self, height: u64) -> usize;
}

pub trait BlockService: Clone + Send + Sync + 'static {
    type Error: Debug + Display;

    fn build_block(&self, height: u64) -> Result<Vec<u8>, Self::Error>;

    fn verify_block(&self, raw_block: &[u8]) -> Result<(), Self::Error>;
}

#[async_trait]
pub trait Broadcast: Clone + Send + Sync + 'static {
    async fn broadcast_message<'a>(&self, message: IBFTBroadcastMessage<'a>);

    async fn broadcast_block(&self, block: &FinalizedBlock);
}
