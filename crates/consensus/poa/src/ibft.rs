use std::cmp;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::select;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tonic_primitives::Signer;
use tracing::{debug, error, info};

use crate::backend::{BlockBuilder, BlockVerifier, Broadcast, ValidatorManager};
use crate::types::{
    CommitMessage, CommitMessageSigned, CommitSeals, FinalizedBlock, IBFTBroadcastMessage,
    PrepareMessage, PrepareMessageSigned, ProposalMessage, ProposalMessageSigned,
};

use super::messages::ConsensusMessages;
use super::types::View;

const TIMEOUT_MULTIPLIER: [u32; 12] = [1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048];

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
    base_round_time: Duration,
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
        base_round_time: Duration,
    ) -> Self {
        Self {
            messages,
            validator_manager,
            broadcast,
            block_verifier,
            block_builder,
            signer,
            base_round_time,
        }
    }

    pub async fn run(
        &self,
        height: u64,
        mut cancel: oneshot::Receiver<()>,
    ) -> Option<FinalizedBlock> {
        let mut view = View { height, round: 0 };

        info!("Running consensus height {}", view.height);
        loop {
            info!("Running consensus round {}", view.round);

            let state = SharedRunState::new(view);

            let timeout = tokio::time::sleep(self.get_round_timeout(view.round));
            let (future_proposal_rx, future_proposal_task) = self.watch_future_proposal(view);
            let (rcc_rx, rcc_task) = self.watch_rcc(view);
            let (round_finished, round_task) = self.start_ibft_round(state.clone());

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
                    return None;
                }
                _ = timeout => {
                    info!("Round timeout");
                    abort();

                    self.handle_timeout(view, state.get().await).await;
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
                Ok(commit_seals) = round_finished => {
                    info!("Finished IBFT round");
                    abort();

                    let proposed_block = self
                        .messages
                        .take_proposal_message(view)
                        .await
                        .expect("Proposal must exist when round finished")
                        .into_proposed_block();
                    return Some(FinalizedBlock::new(proposed_block, commit_seals));
                }
            }
        }
    }

    fn start_ibft_round(
        &self,
        state: SharedRunState,
    ) -> (oneshot::Receiver<CommitSeals>, JoinHandle<()>) {
        let ibft = self.clone();
        let (tx, rx) = oneshot::channel();

        let task = tokio::spawn(async move {
            match ibft.run_ibft_round0(state).await {
                Ok(finalized_block) => {
                    let _ = tx.send(finalized_block);
                }
                Err(err) => {
                    // TODO: think about what to do here
                    error!("Error occurred during IBFT run: {err}");
                }
            }
        });

        (rx, task)
    }

    async fn run_ibft_round0(&self, state: SharedRunState) -> Result<CommitSeals, IBFTError> {
        let view = state.view;

        assert_eq!(view.round, 0, "Round must be 0");
        assert_eq!(
            state.get().await,
            RunState::Proposal,
            "Initial run state must be Proposal"
        );

        let should_propose = self
            .validator_manager
            .is_proposer(self.signer.address(), view);
        let proposed_block_digest = if should_propose {
            info!("We are the block proposer");

            let raw_eth_block = self
                .block_builder
                .build_block(view.height)
                .map_err(IBFTError::BlockBuild)?;
            debug!("Built the proposal block");

            let proposal =
                ProposalMessage::new(view, raw_eth_block, None).into_signed(&self.signer);
            let proposed_block_digest = proposal.proposed_block_digest();

            self.broadcast
                .broadcast_message(IBFTBroadcastMessage::Proposal(&proposal))
                .await;

            self.messages.add_proposal_message(proposal).await;

            proposed_block_digest
        } else {
            let proposal_verify_fn = |proposal: &ProposalMessageSigned| {
                if !proposal.verify_digest() {
                    return Err(IBFTError::IncorrectProposalDigest);
                }

                if let Err(err) = self
                    .block_verifier
                    .verify_block(proposal.proposed_block().raw_eth_block())
                {
                    return Err(IBFTError::InvalidProposalBlock(err));
                }

                Ok(())
            };

            // First subscribe so we don't miss the notification in the brief time we query the proposal.
            let mut proposal_rx = self.messages.subscribe_proposal();
            if let Some(digest) = self
                .messages
                .get_valid_proposal_digest(view, proposal_verify_fn)
                .await
            {
                digest?
            } else {
                // Wait until we receive a proposal from peers for the given view
                loop {
                    let proposal_view = proposal_rx
                        .recv()
                        .await
                        .expect("Proposal subscriber channel should not close");
                    if proposal_view == view {
                        break;
                    }
                }

                let digest = self
                    .messages
                    .get_valid_proposal_digest(view, proposal_verify_fn)
                    .await
                    .expect(
                        "At this state, nothing else should be pruning or taking the proposal",
                    )?;
                debug!("Received valid proposal message");

                let prepare = PrepareMessage::new(view, digest).into_signed(&self.signer);

                self.broadcast
                    .broadcast_message(IBFTBroadcastMessage::Prepare(&prepare))
                    .await;

                self.messages
                    .add_prepare_message(prepare, self.signer.address())
                    .await;

                digest
            }
        };

        state.update(RunState::Prepare).await;
        debug!("Moved to prepare state");

        let quorum = self.validator_manager.quorum(view.height);
        assert_ne!(quorum, 0, "Quorum must be greater than 0");

        // We only need to verify the proposed block digest, signature check is enforced by `MessageHandler`,
        // and querying by view also ensures height and round matches.
        let verify_prepare_fn = |prepare: &PrepareMessageSigned| -> bool {
            prepare.proposed_block_digest() == proposed_block_digest
        };
        let mut prepare_rx = self.messages.subscribe_prepare();
        let mut prepare_count = self
            .messages
            .get_valid_prepare_count(view, verify_prepare_fn)
            .await;
        // Wait for new prepare messages until we hit quorum - 1 (quorum - 1 because proposer does not broadcast prepare)
        while prepare_count < quorum - 1 {
            let new_prepare_view = prepare_rx
                .recv()
                .await
                .expect("Prepare subscriber channel should not close");
            if new_prepare_view == view {
                prepare_count += 1;
                if prepare_count == quorum - 1 {
                    prepare_count = self
                        .messages
                        .get_valid_prepare_count(view, verify_prepare_fn)
                        .await;
                }
            }
        }
        debug!("Received quorum prepare messages");

        let commit =
            CommitMessage::new(view, proposed_block_digest, &self.signer).into_signed(&self.signer);

        self.broadcast
            .broadcast_message(IBFTBroadcastMessage::Commit(&commit))
            .await;

        self.messages
            .add_commit_message(commit, self.signer.address())
            .await;

        state.update(RunState::Commit).await;
        debug!("Moved to commit state");

        // We only need to verify the proposed block digest, signature check is enforced by `MessageHandler`,
        // and querying by view also ensures height and round matches.
        let verify_commit_fn = |commit: &CommitMessageSigned| -> bool {
            commit.proposed_block_digest() == proposed_block_digest
        };
        let mut commit_rx = self.messages.subscribe_commit();
        let (mut commit_seals, mut commit_count) = self
            .messages
            .get_valid_commit_seals(view, verify_commit_fn, quorum)
            .await;
        // Wait for new commit messages until we hit quorum
        while commit_count < quorum {
            let new_commit_view = commit_rx
                .recv()
                .await
                .expect("Commit subscriber channel should not close");
            if new_commit_view == view {
                commit_count += 1;
                if commit_count == quorum {
                    (commit_seals, commit_count) = self
                        .messages
                        .get_valid_commit_seals(view, verify_commit_fn, quorum)
                        .await;
                }
            }
        }
        debug!("Received quorum commit messages");

        Ok(commit_seals.expect("Must have value when reached quorum"))
    }

    fn watch_rcc(&self, _view: View) -> (oneshot::Receiver<()>, JoinHandle<()>) {
        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            // ibft.wait_until_rcc(state).await;
            tokio::time::sleep(Duration::from_secs(9999)).await;
            let _ = tx.send(());
        });

        (rx, task)
    }

    fn watch_future_proposal(&self, _view: View) -> (oneshot::Receiver<()>, JoinHandle<()>) {
        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            // TODO: actually watch for future proposal
            tokio::time::sleep(Duration::from_secs(9999)).await;
            let _ = tx.send(());
        });

        (rx, task)
    }

    async fn handle_timeout(&self, view: View, state: RunState) {
        if state > RunState::Prepare {
            // if last run got past prepare stage, we now have a new prepared proposed
        } else {
            // use the last known prepared proposed
        }
    }

    fn get_round_timeout(&self, round: u32) -> Duration {
        let idx = cmp::min(round, TIMEOUT_MULTIPLIER.len() as u32 - 1);
        self.base_round_time
            .saturating_mul(TIMEOUT_MULTIPLIER[idx as usize])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
enum RunState {
    Proposal = 0,
    Prepare = 1,
    Commit = 2,
}

impl RunState {
    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Proposal,
            1 => Self::Prepare,
            2 => Self::Commit,
            _ => panic!("Should never be called with invalid value"),
        }
    }
}

#[derive(Debug, Clone)]
struct SharedRunState {
    view: View,
    run_state: Arc<AtomicU8>,
}

impl SharedRunState {
    fn new(view: View) -> Self {
        Self {
            view,
            run_state: Arc::new(AtomicU8::new(RunState::Proposal as u8)),
        }
    }

    async fn update(&self, new_state: RunState) {
        self.run_state.store(new_state as u8, Ordering::SeqCst);
    }

    async fn get(&self) -> RunState {
        RunState::from_u8(self.run_state.load(Ordering::SeqCst))
    }
}

#[derive(Debug, thiserror::Error)]
enum IBFTError {
    #[error("Incorrect proposal digest")]
    IncorrectProposalDigest,
    #[error("Invalid proposal block: {0}")]
    InvalidProposalBlock(anyhow::Error),
    #[error("Block builder failed: {0}")]
    BlockBuild(anyhow::Error),
}
