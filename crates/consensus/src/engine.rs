use tokio::sync::{mpsc, oneshot};
use tonic_signer::Signer;

use crate::backend::{BlockVerifier, ValidatorManager};
use crate::ibft::IBFT;
use crate::messages::{ConsensusMessages, MessageHandler};
use crate::types::IBFTMessage;

/// `ConsensusEngine` is the main wrapper that handles synchronization
/// in between incoming P2P messages and the ongoing IBFT run.
pub struct ConsensusEngine<V, BV>
where
    V: ValidatorManager,
    BV: BlockVerifier,
{
    message_handler: MessageHandler<V>,
    ibft: IBFT<V, BV>,
}

impl<V, BV> ConsensusEngine<V, BV>
where
    V: ValidatorManager,
    BV: BlockVerifier,
{
    pub fn new(validator_manager: V, block_verifier: BV, height: u64, signer: Signer) -> Self {
        let messages = ConsensusMessages::new();
        Self {
            message_handler: MessageHandler::new(
                messages.clone(),
                validator_manager.clone(),
                height,
            ),
            ibft: IBFT::new(messages, validator_manager, block_verifier, signer),
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
