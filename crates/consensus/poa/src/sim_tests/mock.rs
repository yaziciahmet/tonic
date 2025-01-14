use async_trait::async_trait;
use tonic_primitives::Address;

use crate::backend::{BlockBuilder, BlockVerifier, Broadcast, ValidatorManager};
use crate::types::{FinalizedBlock, IBFTBroadcastMessage, View};

#[derive(Clone)]
pub struct Mock;

impl ValidatorManager for Mock {
    fn is_proposer(&self, _: Address, _: View) -> bool {
        true
    }

    fn is_validator(&self, _: Address, _: u64) -> bool {
        true
    }

    fn quorum(&self, _: u64) -> usize {
        1
    }
}

#[async_trait]
impl Broadcast for Mock {
    async fn broadcast_message<'a>(&self, _: IBFTBroadcastMessage<'a>) {}

    async fn broadcast_block(&self, _: &FinalizedBlock) {}
}

impl BlockVerifier for Mock {
    type Error = String;
    fn verify_block(&self, _: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl BlockBuilder for Mock {
    type Error = String;
    fn build_block(&self, _: u64) -> Result<Vec<u8>, Self::Error> {
        Ok(vec![1, 2, 3])
    }
}
