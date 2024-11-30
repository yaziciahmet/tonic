use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::anyhow;
use tokio::sync::{mpsc, oneshot};
use tonic_consensus_core::ibft::IBFT;
use tonic_consensus_core::messages::ConsensusMessages;
use tonic_consensus_core::types::{IBFTMessage, View};
use tonic_p2p::IncomingConsensusMessage;
use tonic_primitives::Address;
use tracing::{info, warn};

#[derive(Clone, Debug)]
pub struct ConsensusEngine {
    height: Arc<AtomicU64>,
    messages: ConsensusMessages,
    validators: Vec<Address>,
}

impl ConsensusEngine {
    pub fn new(validators: Vec<Address>) -> Self {
        Self {
            height: Arc::new(AtomicU64::new(0)),
            messages: ConsensusMessages::new(),
            validators,
        }
    }

    pub async fn run(&self, p2p_consensus_rx: mpsc::Receiver<IncomingConsensusMessage>) {
        self.spawn_p2p_message_handler(p2p_consensus_rx);

        let ibft = IBFT::new();
        loop {
            let height = self.height();

            let (_cancel_tx, cancel_rx) = oneshot::channel();
            let _ibft_result = ibft.run(height, &self.messages, cancel_rx).await;

            info!("Finished consensus for height {height}");

            self.messages.prune(height).await;

            self.height.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn spawn_p2p_message_handler(
        &self,
        mut p2p_consensus_rx: mpsc::Receiver<IncomingConsensusMessage>,
    ) {
        let engine = self.clone();

        tokio::spawn(async move {
            loop {
                let (message, peer_id) = p2p_consensus_rx
                    .recv()
                    .await
                    .expect("P2P consensus channel should never be closed");
                if let Err(e) = engine.handle_consensus_message(message).await {
                    warn!("Invalid consensus message by peer {peer_id}: {e}");
                }
            }
        });
    }

    async fn handle_consensus_message(&self, message: IBFTMessage) -> anyhow::Result<()> {
        match message {
            IBFTMessage::Proposal(proposal) => {
                let sender = proposal.recover_signer()?;
                if proposal.view.height < self.height() {
                    return Ok(());
                }
                if !self.is_proposer(&sender, proposal.view) {
                    return Err(anyhow!("Received proposal from non-proposer"));
                }

                self.messages.add_proposal_message(proposal).await;
            }
            IBFTMessage::Prepare(prepare) => {
                let sender = prepare.recover_signer()?;
                if prepare.view.height < self.height() {
                    return Ok(());
                }
                if !self.is_validator(&sender) {
                    return Err(anyhow!("Message sender is not validator"));
                }

                self.messages.add_prepare_message(prepare, sender).await;
            }
            IBFTMessage::Commit(commit) => {
                let sender = commit.recover_signer()?;
                if commit.view.height < self.height() {
                    return Ok(());
                }
                if !self.is_validator(&sender) {
                    return Err(anyhow!("Message sender is not validator"));
                }

                self.messages.add_commit_message(commit, sender).await;
            }
            IBFTMessage::RoundChange(round_change) => {
                let sender = round_change.recover_signer()?;
                if round_change.view.height < self.height() {
                    return Ok(());
                }
                if !self.is_validator(&sender) {
                    return Err(anyhow!("Message sender is not validator"));
                }

                self.messages
                    .add_round_change_message(round_change, sender)
                    .await;
            }
        };

        Ok(())
    }

    fn is_validator(&self, address: &Address) -> bool {
        self.validators.contains(address)
    }

    fn is_proposer(&self, address: &Address, _view: View) -> bool {
        self.is_validator(address)
    }

    // IBFT 2.0 quorum number is ceil(2n/3)
    fn _quorum(&self) -> usize {
        (self.validators.len() as f64 * 2.0 / 3.0).ceil() as usize
    }

    fn height(&self) -> u64 {
        self.height.load(Ordering::Relaxed)
    }
}
