use anyhow::anyhow;
use tokio::select;
use tokio::sync::{mpsc, oneshot};
use tonic_consensus_core::ibft::IBFT;
use tonic_consensus_core::messages::ConsensusMessages;
use tonic_consensus_core::types::IBFTMessage;
use tonic_p2p::IncomingConsensusMessage;
use tonic_primitives::Address;
use tracing::{info, warn};

pub struct ConsensusEngine {
    p2p_consensus_rx: mpsc::Receiver<IncomingConsensusMessage>,
    ibft: IBFT,
    messages: ConsensusMessages,
    validators: Vec<Address>,
}

impl ConsensusEngine {
    pub fn new(
        p2p_consensus_rx: mpsc::Receiver<IncomingConsensusMessage>,
        validators: Vec<Address>,
    ) -> Self {
        let ibft = IBFT::new();
        let messages = ConsensusMessages::new();

        Self {
            p2p_consensus_rx,
            ibft,
            messages,
            validators,
        }
    }

    pub async fn run(&mut self) {
        let mut height = 0;
        loop {
            let (_cancel_tx, cancel_rx) = oneshot::channel();
            let ibft_result = self.ibft.run(height, &self.messages, cancel_rx);

            select! {
                Some((message, peer_id)) = self.p2p_consensus_rx.recv() => {
                    if let Err(e) = self.handle_consensus_message(message, height).await {
                        warn!("Invalid consensus message by peer {peer_id}: {e}");
                    }
                }
                _ = ibft_result => {
                    info!("Finished consensus for height {height}");
                    height += 1;
                    self.messages.prune(height);
                }
            }
        }
    }

    async fn handle_consensus_message(
        &mut self,
        message: IBFTMessage,
        height: u64,
    ) -> anyhow::Result<()> {
        match message {
            IBFTMessage::Proposal(proposal) => {
                let sender = proposal.recover_signer()?;
                if !self.is_validator(&sender) {
                    return Err(anyhow!("Message sender is not validator"));
                }
                if proposal.view.height < height {
                    return Err(anyhow!("Message height is lower than the current state"));
                }

                self.messages.add_proposal_message(proposal, sender);
            }
            IBFTMessage::Prepare(prepare) => {
                let sender = prepare.recover_signer()?;
                if !self.is_validator(&sender) {
                    return Err(anyhow!("Message sender is not validator"));
                }
                if prepare.view.height < height {
                    return Err(anyhow!("Message height is lower than the current state"));
                }

                self.messages.add_prepare_message(prepare, sender)
            }
            IBFTMessage::Commit(commit) => {
                let sender = commit.recover_signer()?;
                if !self.is_validator(&sender) {
                    return Err(anyhow!("Message sender is not validator"));
                }
                if commit.view.height < height {
                    return Err(anyhow!("Message height is lower than the current state"));
                }

                self.messages.add_commit_message(commit, sender)
            }
            IBFTMessage::RoundChange(round_change) => {
                let sender = round_change.recover_signer()?;
                if !self.is_validator(&sender) {
                    return Err(anyhow!("Message sender is not validator"));
                }
                if round_change.view.height < height {
                    return Err(anyhow!("Message height is lower than the current state"));
                }

                self.messages.add_round_change_message(round_change, sender)
            }
        };

        Ok(())
    }

    fn is_validator(&self, address: &Address) -> bool {
        self.validators.contains(address)
    }

    // IBFT 2.0 quorum number is ceil(2n/3)
    fn quorum(&self) -> usize {
        (self.validators.len() as f64 * 2.0 / 3.0).ceil() as usize
    }
}
