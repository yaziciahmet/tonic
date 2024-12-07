use tokio::sync::{mpsc, oneshot};
use tonic_primitives::Address;

use crate::ibft::IBFT;
use crate::messages::{ConsensusMessages, MessageHandler};
use crate::types::IBFTMessage;
use crate::validator_manager::ValidatorManager;

/// `ConsensusEngine` is the main wrapper that handles synchronization
/// in between incoming P2P messages and the ongoing IBFT run.
pub struct ConsensusEngine<V>
where
    V: ValidatorManager,
{
    message_handler: MessageHandler<V>,
    ibft: IBFT<V>,
}

impl<V> ConsensusEngine<V>
where
    V: ValidatorManager,
{
    pub fn new(validator_manager: V, height: u64, address: Address) -> Self {
        let messages = ConsensusMessages::new();
        Self {
            message_handler: MessageHandler::new(
                messages.clone(),
                validator_manager.clone(),
                height,
            ),
            ibft: IBFT::new(messages, validator_manager, address),
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
