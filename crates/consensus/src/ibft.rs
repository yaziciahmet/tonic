use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use tokio::select;
use tokio::sync::{oneshot, RwLock};
use tokio::task::JoinHandle;
use tonic_signer::Signer;
use tracing::info;

use crate::backend::{BlockBuilder, BlockVerifier, Broadcast, ValidatorManager};
use crate::types::{IBFTMessage, ProposalMessage};

use super::messages::ConsensusMessages;
use super::types::View;

const TIMEOUT_TABLE: [Duration; 6] = [
    Duration::from_secs(4),
    Duration::from_secs(8),
    Duration::from_secs(16),
    Duration::from_secs(32),
    Duration::from_secs(64),
    Duration::from_secs(128),
];

#[derive(Clone)]
pub struct IBFT<V, B, BV, BB>
where
    V: ValidatorManager,
    B: Broadcast,
    BV: BlockVerifier,
    BB: BlockBuilder,
{
    messages: ConsensusMessages,
    validator_manager: V,
    broadcast: B,
    signer: Signer,
    block_verifier: BV,
    block_builder: BB,
}

impl<V, B, BV, BB> IBFT<V, B, BV, BB>
where
    V: ValidatorManager,
    B: Broadcast,
    BV: BlockVerifier,
    BB: BlockBuilder,
{
    pub fn new(
        messages: ConsensusMessages,
        validator_manager: V,
        broadcast: B,
        block_verifier: BV,
        block_builder: BB,
        signer: Signer,
    ) -> Self {
        Self {
            messages,
            validator_manager,
            broadcast,
            block_verifier,
            block_builder,
            signer,
        }
    }

    pub async fn run(&self, height: u64, mut cancel: oneshot::Receiver<()>) {
        let mut view = View { height, round: 0 };

        info!("Running consensus height {}", view.height);
        loop {
            info!("Running consensus round {}", view.round);

            let state = SharedRunState::new(view);

            let timeout = tokio::time::sleep(get_round_timeout(view.round));
            let (future_proposal_rx, future_proposal_task) =
                self.watch_future_proposal(state.clone());
            let (rcc_rx, rcc_task) = self.watch_rcc(state.clone());
            let (round_finished, round_task) = self.start_ibft_round(state);

            let abort = move || {
                round_task.abort();
                future_proposal_task.abort();
                rcc_task.abort();
            };

            select! {
                biased;
                _ = &mut cancel => {
                    info!("Received cancel signal, stopping consensus...");
                    abort();
                    return;
                }
                _ = timeout => {
                    info!("Round timeout");
                    abort();
                    view.round += 1;
                }
                _ = future_proposal_rx => {
                    info!("Received future proposal");
                    abort();
                }
                _ = rcc_rx => {
                    info!("Got enough round change messages to create round change certificate");
                    abort();
                }
                _ = round_finished => {
                    info!("Finished IBFT round");
                    abort();
                    return;
                }
            }
        }
    }

    fn start_ibft_round(&self, state: SharedRunState) -> (oneshot::Receiver<()>, JoinHandle<()>) {
        let ibft = self.clone();
        let (tx, rx) = oneshot::channel();

        let task = tokio::spawn(async move {
            let _ = ibft.run_ibft_round0(state).await;
            let _ = tx.send(());
        });

        (rx, task)
    }

    async fn run_ibft_round0(&self, state: SharedRunState) -> Result<(), IBFTError> {
        let view = state.view;

        assert_eq!(view.round, 0, "round must be 0");

        let proposal = if self
            .validator_manager
            .is_proposer(self.signer.address(), view)
        {
            info!("We are the block proposer");

            // Build a block
            let raw_eth_block = self.block_builder.build_block(view.height)?;
            let proposal =
                Arc::new(ProposalMessage::new(view, raw_eth_block, None).into_signed(&self.signer));

            // Add the proposal to messages and broadcast it to peers
            self.messages.add_proposal_message(proposal.clone()).await;
            self.broadcast
                .broadcast(IBFTMessage::Proposal(proposal.clone()))?;

            proposal
        } else {
            // We first subscribe so we don't miss the notification in the brief time we query the proposal.
            let mut proposal_rx = self.messages.subscribe_proposal();
            let proposal = if let Some(proposal) = self.messages.get_proposal_message(view).await {
                proposal
            } else {
                // Wait until we receive a proposal from peers for the given view
                loop {
                    let proposal = proposal_rx
                        .recv()
                        .await
                        .expect("Proposal subscriber channel should not close");
                    if proposal.view() == view {
                        break proposal;
                    }
                }
            };

            let proposed_block = proposal.proposed_block();
            // Verify proposed block's round
            if proposed_block.round() != view.round {
                return Err(IBFTError::IncorrectProposedBlockRound);
            }
            // Verify proposed block digest
            if !proposal.verify_digest() {
                return Err(IBFTError::IncorrectProposalDigest);
            }
            // Verify ethereum block
            if let Err(err) = self
                .block_verifier
                .verify_block(proposed_block.raw_eth_block())
            {
                return Err(IBFTError::InvalidBlock(err));
            }

            proposal
        };

        state.set_state(RunState::Prepare).await;

        Ok(())
    }

    fn watch_rcc(&self, _state: SharedRunState) -> (oneshot::Receiver<()>, JoinHandle<()>) {
        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            // TODO: actually watch for rcc
            tokio::time::sleep(Duration::from_secs(9999)).await;
            let _ = tx.send(());
        });

        (rx, task)
    }

    fn watch_future_proposal(
        &self,
        _state: SharedRunState,
    ) -> (oneshot::Receiver<()>, JoinHandle<()>) {
        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            // TODO: actually watch for future proposal
            tokio::time::sleep(Duration::from_secs(9999)).await;
            let _ = tx.send(());
        });

        (rx, task)
    }
}

fn get_round_timeout(mut round: u32) -> Duration {
    if round > 5 {
        round = 5;
    }

    TIMEOUT_TABLE[round as usize]
}

/// `SharedRunState` is the shared state of the currently running IBFT
/// consensus. Used for tracking the current step of the IBFT run.
#[derive(Clone, Debug)]
struct SharedRunState {
    view: View,
    state: Arc<RwLock<RunState>>,
}

impl SharedRunState {
    fn new(view: View) -> Self {
        Self {
            view,
            state: Default::default(),
        }
    }

    async fn set_state(&self, new_state: RunState) {
        *self.state.write().await = new_state;
    }

    async fn accepted_proposal(&self) -> bool {
        *self.state.read().await != RunState::Proposal
    }
}

#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
enum RunState {
    #[default]
    Proposal,
    Prepare,
    Commit,
    Finalized,
}

#[derive(Debug, thiserror::Error)]
enum IBFTError {
    #[error("Incorrect proposed block round")]
    IncorrectProposedBlockRound,
    #[error("Incorrect proposal digest")]
    IncorrectProposalDigest,
    #[error("Invalid block: {0}")]
    InvalidBlock(anyhow::Error),
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}
