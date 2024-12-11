use tokio::sync::{mpsc, oneshot};
use tonic_signer::Signer;

use crate::backend::{BlockBuilder, BlockVerifier, ValidatorManager};
use crate::ibft::IBFT;
use crate::messages::{ConsensusMessages, MessageHandler};
use crate::types::IBFTMessage;

/// `ConsensusEngine` is the main wrapper that handles synchronization
/// in between incoming P2P messages and the ongoing IBFT run.
pub struct ConsensusEngine<V, BV, BB>
where
    V: ValidatorManager,
    BV: BlockVerifier,
    BB: BlockBuilder,
{
    message_handler: MessageHandler<V>,
    ibft: IBFT<V, BV, BB>,
}

impl<V, BV, BB> ConsensusEngine<V, BV, BB>
where
    V: ValidatorManager,
    BV: BlockVerifier,
    BB: BlockBuilder,
{
    pub fn new(
        validator_manager: V,
        block_verifier: BV,
        block_builder: BB,
        height: u64,
        signer: Signer,
    ) -> Self {
        let messages = ConsensusMessages::new();
        Self {
            message_handler: MessageHandler::new(
                messages.clone(),
                validator_manager.clone(),
                height,
            ),
            ibft: IBFT::new(
                messages,
                validator_manager,
                block_verifier,
                block_builder,
                signer,
            ),
        }
    }

    /// Runs IBFT consensus for the given height.
    pub async fn run_height(&self, height: u64, cancel_rx: oneshot::Receiver<()>) {
        let _ibft_result = self.ibft.run(height, cancel_rx).await;

        self.message_handler.update_height(height);
        self.message_handler.prune().await;
    }

    /// Spawns a background tokio task which handles incoming P2P consensus messages.
    pub fn spawn_message_handler(&self, p2p_rx: mpsc::Receiver<IBFTMessage>) {
        let message_handler = self.message_handler.clone();
        tokio::spawn(message_handler.start(p2p_rx));
    }
}
