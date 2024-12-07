use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::anyhow;
use tokio::sync::{mpsc, oneshot};
use tonic_consensus_core::ibft::IBFT;
use tonic_consensus_core::messages::ConsensusMessages;
use tonic_consensus_core::types::IBFTMessage;
use tonic_consensus_core::validator_manager::ValidatorManager;
use tonic_p2p::IncomingConsensusMessage;
use tonic_primitives::Address;
use tracing::warn;

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
    pub fn new(validator_manager: V, height: u64, self_address: Address) -> Self {
        let messages = ConsensusMessages::new();
        Self {
            message_handler: MessageHandler::new(
                messages.clone(),
                validator_manager.clone(),
                height,
            ),
            ibft: IBFT::new(messages, validator_manager, self_address),
        }
    }

    pub async fn run_height(&self, height: u64, cancel_rx: oneshot::Receiver<()>) {
        let _ibft_result = self.ibft.run(height, cancel_rx).await;

        self.message_handler.update_height(height);
        self.message_handler.prune().await;
    }

    pub fn spawn_message_handler(&self, p2p_rx: mpsc::Receiver<IncomingConsensusMessage>) {
        let message_handler = self.message_handler.clone();
        tokio::spawn(message_handler.start(p2p_rx));
    }
}

#[derive(Clone, Debug)]
struct MessageHandler<V>
where
    V: ValidatorManager,
{
    messages: ConsensusMessages,
    validator_manager: V,
    height: Arc<AtomicU64>,
}

impl<V> MessageHandler<V>
where
    V: ValidatorManager,
{
    fn new(messages: ConsensusMessages, validator_manager: V, height: u64) -> Self {
        Self {
            messages,
            validator_manager,
            height: Arc::new(AtomicU64::new(height)),
        }
    }

    /// Starts listening from p2p receiver channel for new messages.
    /// Consumes self and should be used in `tokio::spawn`.
    async fn start(self, mut p2p_rx: mpsc::Receiver<IncomingConsensusMessage>) {
        loop {
            let (message, peer_id) = p2p_rx
                .recv()
                .await
                .expect("P2P consensus channel should never be closed");
            if let Err(e) = self.handle_consensus_message(message).await {
                warn!("Invalid consensus message by peer {peer_id}: {e}");
            }
        }
    }

    async fn handle_consensus_message(&self, message: IBFTMessage) -> anyhow::Result<()> {
        match message {
            IBFTMessage::Proposal(proposal) => {
                if proposal.view.height <= self.height() {
                    return Ok(());
                }

                let sender = proposal.recover_signer()?;
                if !self.validator_manager.is_proposer(sender, proposal.view) {
                    return Err(anyhow!("Received proposal from non-proposer"));
                }

                self.messages.add_proposal_message(proposal).await;
            }
            IBFTMessage::Prepare(prepare) => {
                if prepare.view.height <= self.height() {
                    return Ok(());
                }

                let sender = prepare.recover_signer()?;
                if !self
                    .validator_manager
                    .is_validator(sender, prepare.view.height)
                {
                    return Err(anyhow!("Message sender is not validator"));
                }

                self.messages.add_prepare_message(prepare, sender).await;
            }
            IBFTMessage::Commit(commit) => {
                if commit.view.height <= self.height() {
                    return Ok(());
                }

                let sender = commit.recover_signer()?;
                if !self
                    .validator_manager
                    .is_validator(sender, commit.view.height)
                {
                    return Err(anyhow!("Message sender is not validator"));
                }

                self.messages.add_commit_message(commit, sender).await;
            }
            IBFTMessage::RoundChange(round_change) => {
                if round_change.view.height <= self.height() {
                    return Ok(());
                }

                let sender = round_change.recover_signer()?;
                if !self
                    .validator_manager
                    .is_validator(sender, round_change.view.height)
                {
                    return Err(anyhow!("Message sender is not validator"));
                }

                self.messages
                    .add_round_change_message(round_change, sender)
                    .await;
            }
        };

        Ok(())
    }

    /// Updates known finalized height. Panics if the new height is less than or equal to current finalized height.
    fn update_height(&self, height: u64) {
        assert!(
            height > self.height(),
            "Updated height must always be higher than the current height"
        );
        self.height.store(height, Ordering::Relaxed);
    }

    /// Prunes messages that are for height lower than finalized height, including finalized height.
    async fn prune(&self) {
        self.messages.prune(self.height() + 1).await;
    }

    /// Returns the finalized height.
    fn height(&self) -> u64 {
        self.height.load(Ordering::Relaxed)
    }
}
