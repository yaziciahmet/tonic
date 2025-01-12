pub mod backend;
pub mod codec;
pub mod engine;
pub mod ibft;
pub mod messages;
pub mod types;

/// Total of 25 rounds with 2 seconds base timeout corresponds to about 776 days for a single height.
/// If a block can't be produced for 776 days, it is safe to assume that the chain is dead.
pub(crate) const MAX_ROUND: u8 = 24;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_trait::async_trait;
    use tokio::sync::oneshot;
    use tonic_primitives::{Address, Signer};

    use crate::backend::{BlockBuilder, BlockVerifier, Broadcast, ValidatorManager};
    use crate::engine::ConsensusEngine;
    use crate::types::{FinalizedBlock, IBFTBroadcastMessage, View};

    #[derive(Clone)]
    struct Mock;

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
        fn verify_block(&self, _: &[u8]) -> anyhow::Result<()> {
            Ok(())
        }
    }

    impl BlockBuilder for Mock {
        fn build_block(&self, _: u64) -> anyhow::Result<Vec<u8>> {
            Ok(vec![1, 2, 3])
        }
    }

    #[tokio::test]
    async fn ibft_run() {
        tonic_tracing::initialize_tracing(tracing::Level::DEBUG);
        let mock = Mock {};
        let signer = Signer::random();
        let engine = ConsensusEngine::new(
            mock.clone(),
            mock.clone(),
            mock.clone(),
            mock,
            1,
            signer,
            Duration::from_secs(1),
        );

        let (_tx, rx) = oneshot::channel();
        let _finalized_block = engine.run_height(2, rx).await;
        let (_tx, rx) = oneshot::channel();
        let finalized_block = engine.run_height(3, rx).await;
        assert!(finalized_block.is_some());
    }
}
