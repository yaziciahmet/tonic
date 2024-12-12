use tokio::sync::{mpsc, oneshot};
use tonic_signer::Signer;

use crate::backend::{BlockBuilder, BlockVerifier, Broadcast, ValidatorManager};
use crate::ibft::IBFT;
use crate::messages::{ConsensusMessages, MessageHandler};
use crate::types::{FinalizedBlock, IBFTReceivedMessage};

/// `ConsensusEngine` is the main wrapper that handles synchronization
/// in between incoming P2P messages and the ongoing IBFT run.
pub struct ConsensusEngine<V, B, BV, BB>
where
    V: ValidatorManager,
    B: Broadcast,
    BV: BlockVerifier,
    BB: BlockBuilder,
{
    message_handler: MessageHandler<V>,
    ibft: IBFT<V, B, BV, BB>,
}

impl<V, B, BV, BB> ConsensusEngine<V, B, BV, BB>
where
    V: ValidatorManager,
    B: Broadcast,
    BV: BlockVerifier,
    BB: BlockBuilder,
{
    pub fn new(
        validator_manager: V,
        broadcast: B,
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
                broadcast,
                block_verifier,
                block_builder,
                signer,
            ),
        }
    }

    /// Runs IBFT consensus for the given height.
    pub async fn run_height(
        &self,
        height: u64,
        cancel_rx: oneshot::Receiver<()>,
    ) -> Option<FinalizedBlock> {
        let finalized_block = self.ibft.run(height, cancel_rx).await;

        self.message_handler.update_height(height);
        self.message_handler.prune().await;

        finalized_block
    }

    /// Spawns a background tokio task which handles incoming P2P consensus messages.
    pub fn spawn_message_handler(&self, p2p_rx: mpsc::Receiver<IBFTReceivedMessage>) {
        let message_handler = self.message_handler.clone();
        tokio::spawn(message_handler.start(p2p_rx));
    }
}
