use tonic_primitives::Address;

use crate::types::View;

pub trait ValidatorManager: Clone + Send + Sync + 'static {
    fn is_validator(&self, address: Address, height: u64) -> bool;

    fn is_proposer(&self, address: Address, view: View) -> bool;

    fn quorum(&self, height: u64) -> usize;
}

pub trait BlockVerifier: Clone + Send + Sync + 'static {
    fn verify_block(&self, raw_block: &[u8]) -> anyhow::Result<()>;
}

pub trait BlockBuilder: Clone + Send + Sync + 'static {
    fn build_block(&self, height: u64) -> anyhow::Result<Vec<u8>>;
}
