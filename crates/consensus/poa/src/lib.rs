pub mod backend;
pub mod codec;
pub mod engine;
pub mod ibft;
pub mod messages;
#[cfg(all(test, madsim))]
mod sim_tests;
pub mod syncer;
pub mod types;

/// Total of 22 rounds with 2 seconds base timeout corresponds to about 97 days for a single height.
/// If a block can't be produced for 97 days, it is safe to assume that the chain is dead.
pub(crate) const MAX_ROUND: u8 = 21;
/// Const array size for arrays using indices as the round
pub(crate) const ROUND_ARRAY_SIZE: usize = (MAX_ROUND + 1) as usize;
