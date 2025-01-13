use std::collections::{btree_map, hash_map, BTreeMap, HashMap};
use std::mem;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::anyhow;
use tokio::sync::{broadcast, mpsc, Mutex};
use tonic_primitives::Address;
use tracing::warn;

use crate::backend::ValidatorManager;
use crate::types::{CommitSeals, IBFTReceivedMessage};
use crate::{MAX_ROUND, ROUND_ARRAY_SIZE};

use super::types::{
    CommitMessageSigned, PrepareMessageSigned, ProposalMessageSigned, RoundChangeMessageSigned,
    View,
};

const CHANNEL_SIZE: usize = 128;

/// `MessageHandler` is the P2P message handler. All incoming messages are stored
/// in `ConsensusMessages`. It also ensures messages are sent by validators for
/// the given view.
#[derive(Clone, Debug)]
pub struct MessageHandler<V>
where
    V: ValidatorManager,
{
    messages: ConsensusMessages,
    validator_manager: V,
    height: Arc<AtomicU64>,
    address: Address,
}

impl<V> MessageHandler<V>
where
    V: ValidatorManager,
{
    pub fn new(
        messages: ConsensusMessages,
        validator_manager: V,
        height: u64,
        address: Address,
    ) -> Self {
        Self {
            messages,
            validator_manager,
            height: Arc::new(AtomicU64::new(height)),
            address,
        }
    }

    /// Starts listening from p2p receiver channel for new messages.
    /// Consumes self and should be used in `tokio::spawn`.
    pub async fn start(self, mut p2p_rx: mpsc::Receiver<IBFTReceivedMessage>) {
        loop {
            let message = p2p_rx
                .recv()
                .await
                .expect("P2P consensus channel should never be closed");
            if let Err(e) = self.handle_consensus_message(message).await {
                warn!("Invalid consensus message by peer: {e}");
            }
        }
    }

    /// Handle incoming p2p message. Verifies that the message sender is a validator,
    /// and if proposal, the message sender is proposer. For commit messages, it also
    /// verifies that the commit seal signer and message signer matches.
    async fn handle_consensus_message(&self, message: IBFTReceivedMessage) -> anyhow::Result<()> {
        match message {
            IBFTReceivedMessage::Proposal(proposal) => {
                let view = proposal.view();

                if view.height <= self.height() {
                    return Ok(());
                }
                if view.round > MAX_ROUND {
                    return Err(anyhow!("Received proposal with too high round"));
                }

                let sender = proposal.recover_signer()?;
                if sender == self.address {
                    return Err(anyhow!("Received self signed proposal"));
                }

                if !self.validator_manager.is_proposer(sender, view) {
                    return Err(anyhow!("Received proposal from non-proposer"));
                }

                self.messages.add_proposal_message(proposal).await;
            }
            IBFTReceivedMessage::Prepare(prepare) => {
                let view = prepare.view();

                if view.height <= self.height() {
                    return Ok(());
                }
                if view.round > MAX_ROUND {
                    return Err(anyhow!("Received prepare with too high round"));
                }

                let sender = prepare.recover_signer()?;
                if sender == self.address {
                    return Err(anyhow!("Received self signed prepare"));
                }

                if !self.validator_manager.is_validator(sender, view.height) {
                    return Err(anyhow!("Prepare sender is not validator"));
                }

                if self.validator_manager.is_proposer(sender, view) {
                    return Err(anyhow!("Proposer should not send prepare messages"));
                }

                self.messages.add_prepare_message(prepare, sender).await;
            }
            IBFTReceivedMessage::Commit(commit) => {
                let view = commit.view();

                if view.height <= self.height() {
                    return Ok(());
                }
                if view.round > MAX_ROUND {
                    return Err(anyhow!("Received commit with too high round"));
                }

                let sender = commit.recover_signer()?;
                if sender == self.address {
                    return Err(anyhow!("Received self signed commit"));
                }

                if !self.validator_manager.is_validator(sender, view.height) {
                    return Err(anyhow!("Commit sender is not validator"));
                }

                let seal_sender = commit.recover_commit_seal_signer()?;
                if sender != seal_sender {
                    return Err(anyhow!(
                        "Commit seal signer is different from message signer"
                    ));
                }

                self.messages.add_commit_message(commit, sender).await;
            }
            IBFTReceivedMessage::RoundChange(round_change) => {
                let view = round_change.view();

                if view.height <= self.height() {
                    return Ok(());
                }
                if view.round == 0 {
                    return Err(anyhow!("Received round change with round 0"));
                }
                if view.round > MAX_ROUND {
                    return Err(anyhow!("Received round change with too high round"));
                }

                let sender = round_change.recover_signer()?;
                if sender == self.address {
                    return Err(anyhow!("Received self signed round change"));
                }

                if !self.validator_manager.is_validator(sender, view.height) {
                    return Err(anyhow!("Round change sender is not validator"));
                }

                self.messages
                    .add_round_change_message(round_change, sender)
                    .await;
            }
        };

        Ok(())
    }

    /// Updates known finalized height. Panics if the new height is less than or equal to current finalized height.
    pub fn update_height(&self, height: u64) {
        assert!(
            height > self.height(),
            "Updated height must always be higher than the current height"
        );
        self.height.store(height, Ordering::Relaxed);
    }

    /// Prunes messages that are for height lower than finalized height, including finalized height.
    pub async fn prune(&self) {
        self.messages.prune(self.height() + 1).await;
    }

    /// Returns the finalized height.
    fn height(&self) -> u64 {
        self.height.load(Ordering::Relaxed)
    }
}

/// Container for consensus messages received by peers. `ConsensusMessages` ensures that:
/// - no duplicate messages from the same sender for the same view; only the first is accepted, and the rest are discarded.
///
/// Certain checks must be done before adding a message:
/// - message has a valid signature
/// - message signer is a valid validator for the corresponding height
/// - if proposal, signer must be the proposer for the corresponding height and round
///
/// Also provides subscription capabilities.
#[derive(Clone, Debug)]
pub struct ConsensusMessages {
    self_address: Address,

    proposal_messages: Arc<Mutex<ViewMap<ProposalMessageSigned>>>,
    prepare_messages: Arc<Mutex<ViewSenderMap<PrepareMessageSigned>>>,
    commit_messages: Arc<Mutex<ViewSenderMap<CommitMessageSigned>>>,
    // Caching also whether round change is verified since verifying round changes are expensive.
    round_change_messages: Arc<Mutex<ViewSenderMap<(RoundChangeMessageSigned, bool)>>>,

    proposal_tx: broadcast::Sender<View>,
    prepare_tx: broadcast::Sender<View>,
    commit_tx: broadcast::Sender<View>,
    round_change_tx: broadcast::Sender<View>,
}

impl ConsensusMessages {
    pub fn new(address: Address) -> Self {
        Self {
            self_address: address,
            proposal_messages: Arc::new(Mutex::new(ViewMap::new())),
            prepare_messages: Arc::new(Mutex::new(ViewSenderMap::new())),
            commit_messages: Arc::new(Mutex::new(ViewSenderMap::new())),
            round_change_messages: Arc::new(Mutex::new(ViewSenderMap::new())),
            proposal_tx: broadcast::channel(CHANNEL_SIZE).0,
            prepare_tx: broadcast::channel(CHANNEL_SIZE).0,
            commit_tx: broadcast::channel(CHANNEL_SIZE).0,
            round_change_tx: broadcast::channel(CHANNEL_SIZE).0,
        }
    }

    /// Prunes messages less than height
    pub async fn prune(&self, height: u64) {
        // Prune proposal
        let mut proposal_messages = self.proposal_messages.lock().await;
        proposal_messages.prune(height);
        drop(proposal_messages);
        // Prune prepare
        let mut prepare_messages = self.prepare_messages.lock().await;
        prepare_messages.prune(height);
        drop(prepare_messages);
        // Prune commit
        let mut commit_messages = self.commit_messages.lock().await;
        commit_messages.prune(height);
        drop(commit_messages);
        // Prune round changes
        let mut round_change_messages = self.round_change_messages.lock().await;
        round_change_messages.prune(height);
        drop(round_change_messages);
    }

    /// Adds a proposal message if not already exists for the view, and broadcasts it.
    pub async fn add_proposal_message(&self, proposal: ProposalMessageSigned) {
        let view = proposal.view();
        let mut proposal_messages = self.proposal_messages.lock().await;
        let entry = proposal_messages.view_entry(view);
        if let btree_map::Entry::Vacant(entry) = entry {
            entry.insert(proposal);
            let _ = self.proposal_tx.send(view);
        }
    }

    /// Adds a prepare message if not already exists for the view and sender, and broadcasts it.
    pub async fn add_prepare_message(&self, prepare: PrepareMessageSigned, sender: Address) {
        let view = prepare.view();
        let mut prepare_messages = self.prepare_messages.lock().await;
        let entry = prepare_messages.sender_entry(view, sender);
        if let hash_map::Entry::Vacant(entry) = entry {
            entry.insert(prepare);
            let _ = self.prepare_tx.send(view);
        }
    }

    /// Adds a commit message if not already exists for the view and sender, and broadcasts it.
    pub async fn add_commit_message(&self, commit: CommitMessageSigned, sender: Address) {
        let view = commit.view();
        let mut commit_messages = self.commit_messages.lock().await;
        let entry = commit_messages.sender_entry(view, sender);
        if let hash_map::Entry::Vacant(entry) = entry {
            entry.insert(commit);
            let _ = self.commit_tx.send(view);
        }
    }

    /// Adds a round change message if not already exists for the view and sender, and broadcasts it.
    pub async fn add_round_change_message(
        &self,
        round_change: RoundChangeMessageSigned,
        sender: Address,
    ) {
        let view = round_change.view();
        let mut round_change_messages = self.round_change_messages.lock().await;
        let entry = round_change_messages.sender_entry(view, sender);
        if let hash_map::Entry::Vacant(entry) = entry {
            // Insert as valid if coming from self
            entry.insert((round_change, sender == self.self_address));
            let _ = self.round_change_tx.send(view);
        }
    }

    /// Subscribe to all newly incoming proposal messages.
    pub fn subscribe_proposal(&self) -> broadcast::Receiver<View> {
        self.proposal_tx.subscribe()
    }

    /// Subscribe to all newly incoming prepare messages.
    pub fn subscribe_prepare(&self) -> broadcast::Receiver<View> {
        self.prepare_tx.subscribe()
    }

    /// Subscribe to all newly incoming commit messages.
    pub fn subscribe_commit(&self) -> broadcast::Receiver<View> {
        self.commit_tx.subscribe()
    }

    /// Subscribe to all newly incoming round change messages.
    pub fn subscribe_round_change(&self) -> broadcast::Receiver<View> {
        self.round_change_tx.subscribe()
    }

    /// Verifies the proposal message for the given view with the given verify_fn, and returns it's digest.
    /// Returns `None` if proposal for the view doesn't exist.
    pub async fn get_valid_proposal_digest<F, E>(
        &self,
        view: View,
        verify_fn: F,
    ) -> Option<Result<[u8; 32], E>>
    where
        F: Fn(&ProposalMessageSigned) -> Result<(), E>,
    {
        let mut proposal_messages = self.proposal_messages.lock().await;
        match proposal_messages.view_entry(view) {
            btree_map::Entry::Vacant(_) => None,
            btree_map::Entry::Occupied(entry) => {
                let proposal = entry.get();
                Some(verify_fn(proposal).map(|_| proposal.proposed_block_digest()))
            }
        }
    }

    /// Verifies and prunes the prepare messages for the given view with the given verify_fn, and returns the final count.
    pub async fn get_valid_prepare_count<F>(&self, view: View, verify_fn: F) -> usize
    where
        F: Fn(&PrepareMessageSigned) -> bool,
    {
        let mut prepare_messages = self.prepare_messages.lock().await;
        let messages = prepare_messages.view_entry(view).or_default();

        // Prune invalid messages
        messages.retain(|_, prepare| verify_fn(prepare));

        messages.len()
    }

    /// Verifies and prunes the commit messages for the given view with the given verify_fn.
    /// Consumes commits and returns seals (if valid message count is >= quorum), and returns the valid message count.
    pub async fn take_valid_commit_seals<F>(
        &self,
        view: View,
        verify_fn: F,
        quorum: usize,
    ) -> (Option<CommitSeals>, usize)
    where
        F: Fn(&CommitMessageSigned) -> bool,
    {
        let mut commit_messages = self.commit_messages.lock().await;
        let messages = commit_messages.view_entry(view).or_default();

        // Prune invalid messages
        messages.retain(|_, commit| verify_fn(commit));

        let count = messages.len();
        if count >= quorum {
            let messages = mem::take(messages)
                .into_values()
                .map(|commit| commit.commit_seal())
                .collect();
            (Some(messages), count)
        } else {
            (None, count)
        }
    }

    /// Takes the proposal message for the given view.
    pub async fn take_proposal_message(&self, view: View) -> Option<ProposalMessageSigned> {
        let mut proposal_messages = self.proposal_messages.lock().await;
        match proposal_messages.view_entry(view) {
            btree_map::Entry::Vacant(_) => None,
            btree_map::Entry::Occupied(entry) => Some(entry.remove()),
        }
    }

    /// Takes valid prepare messages for the given view verified with the verify_fn.
    /// Use with caution as this does not do quorum check and removes all prepares.
    pub async fn take_valid_prepare_messages<F>(
        &self,
        view: View,
        verify_fn: F,
    ) -> Vec<PrepareMessageSigned>
    where
        F: Fn(&PrepareMessageSigned) -> bool,
    {
        let mut prepare_messages = self.prepare_messages.lock().await;
        let mut messages = match prepare_messages.view_entry(view) {
            btree_map::Entry::Vacant(_) => return vec![],
            btree_map::Entry::Occupied(entry) => entry.remove(),
        };

        // Prune invalid messages
        messages.retain(|_, prepare| verify_fn(prepare));

        messages.into_values().collect()
    }

    /// Verifies and prunes the round change messages for the given view with the given verify_fn.
    /// Takes round changes (if valid message count is >= quorum), and returns the valid message count.
    pub async fn take_valid_round_change_messages<F>(
        &self,
        view: View,
        verify_fn: F,
        quorum: usize,
    ) -> (Option<Vec<RoundChangeMessageSigned>>, usize)
    where
        F: Fn(&RoundChangeMessageSigned) -> bool,
    {
        let mut round_change_messages = self.round_change_messages.lock().await;
        let messages = round_change_messages.view_entry(view).or_default();

        // Prune invalid messages and mark valid messages
        messages.retain(|_, (round_change, verified)| {
            if *verified {
                return true;
            }

            if !verify_fn(round_change) {
                return false;
            }

            *verified = true;
            return true;
        });

        let count = messages.len();
        if count >= quorum {
            let messages = mem::take(messages)
                .into_values()
                .map(|(round_change, _)| round_change)
                .collect();
            (Some(messages), count)
        } else {
            (None, count)
        }
    }

    /// Returns the message count of each round as index in the resulting array.
    pub async fn round_change_count_by_round(&self, height: u64) -> [usize; ROUND_ARRAY_SIZE] {
        let mut round_change_messages = self.round_change_messages.lock().await;
        let messages_by_round = round_change_messages.height_entry(height).or_default();

        let mut message_count_by_round = [0; ROUND_ARRAY_SIZE];
        for (round, messages) in messages_by_round {
            message_count_by_round[*round as usize] = messages.len();
        }

        message_count_by_round
    }

    /// Verifies and prunes the round change messages for the given view with the given verify_fn, and returns the final count.
    pub async fn get_valid_round_change_count<F>(&self, view: View, verify_fn: F) -> usize
    where
        F: Fn(&RoundChangeMessageSigned) -> bool,
    {
        let mut round_change_mesages = self.round_change_messages.lock().await;
        let messages = round_change_mesages.view_entry(view).or_default();

        // Prune invalid messages and mark valid messages
        messages.retain(|_, (round_change, verified)| {
            if *verified {
                return true;
            }

            if !verify_fn(round_change) {
                return false;
            }

            *verified = true;
            return true;
        });

        messages.len()
    }
}

#[derive(Debug)]
struct ViewSenderMap<T>(ViewMap<HashMap<Address, T>>);

impl<T> Deref for ViewSenderMap<T> {
    type Target = ViewMap<HashMap<Address, T>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for ViewSenderMap<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> ViewSenderMap<T> {
    fn new() -> Self {
        Self(ViewMap::new())
    }

    fn sender_entry(&mut self, view: View, sender: Address) -> hash_map::Entry<'_, Address, T> {
        self.view_entry(view).or_default().entry(sender)
    }
}

#[derive(Debug)]
struct ViewMap<T>(BTreeMap<u64, BTreeMap<u8, T>>);

impl<T> ViewMap<T> {
    fn new() -> Self {
        Self(Default::default())
    }

    // Prune messages less than height
    fn prune(&mut self, height: u64) {
        self.0 = self.0.split_off(&height);
    }

    fn view_entry(&mut self, view: View) -> btree_map::Entry<'_, u8, T> {
        self.0.entry(view.height).or_default().entry(view.round)
    }

    fn height_entry(&mut self, height: u64) -> btree_map::Entry<'_, u64, BTreeMap<u8, T>> {
        self.0.entry(height)
    }
}
