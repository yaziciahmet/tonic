pub mod backend;
pub mod codec;
pub mod engine;
pub mod ibft;
pub mod messages;
pub mod types;

mod tests {
    use async_trait::async_trait;
    use tokio::sync::oneshot;
    use tonic_primitives::Address;
    use tonic_signer::Signer;

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
        tonic::initialize_tracing(tracing::Level::DEBUG);
        let mock = Mock {};
        let signer = Signer::random();
        let engine =
            ConsensusEngine::new(mock.clone(), mock.clone(), mock.clone(), mock, 1, signer);

        let (_tx, rx) = oneshot::channel();
        let finalized_block = engine.run_height(2, rx).await;

        dbg!(finalized_block);
    }
}
