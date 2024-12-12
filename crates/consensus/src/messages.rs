use std::collections::{btree_map, hash_map, BTreeMap, HashMap};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::anyhow;
use tokio::sync::{broadcast, mpsc, Mutex};
use tonic_primitives::{Address, PrimitiveSignature};
use tracing::warn;

use crate::backend::ValidatorManager;
use crate::types::IBFTMessage;

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
}

impl<V> MessageHandler<V>
where
    V: ValidatorManager,
{
    pub fn new(messages: ConsensusMessages, validator_manager: V, height: u64) -> Self {
        Self {
            messages,
            validator_manager,
            height: Arc::new(AtomicU64::new(height)),
        }
    }

    /// Starts listening from p2p receiver channel for new messages.
    /// Consumes self and should be used in `tokio::spawn`.
    pub async fn start(self, mut p2p_rx: mpsc::Receiver<IBFTMessage>) {
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
    async fn handle_consensus_message(&self, message: IBFTMessage) -> anyhow::Result<()> {
        match message {
            IBFTMessage::Proposal(proposal) => {
                let view = proposal.view();

                if view.height <= self.height() {
                    return Ok(());
                }

                let sender = proposal.recover_signer()?;
                if !self.validator_manager.is_proposer(sender, view) {
                    return Err(anyhow!("Received proposal from non-proposer"));
                }

                self.messages.add_proposal_message(proposal).await;
            }
            IBFTMessage::Prepare(prepare) => {
                let view = prepare.view();

                if view.height <= self.height() {
                    return Ok(());
                }

                let sender = prepare.recover_signer()?;
                if !self.validator_manager.is_validator(sender, view.height) {
                    return Err(anyhow!("Message sender is not validator"));
                }

                self.messages.add_prepare_message(prepare, sender).await;
            }
            IBFTMessage::Commit(commit) => {
                let view = commit.view();

                if view.height <= self.height() {
                    return Ok(());
                }

                let sender = commit.recover_signer()?;
                if !self.validator_manager.is_validator(sender, view.height) {
                    return Err(anyhow!("Message sender is not validator"));
                }

                let seal_sender = commit.recover_commit_seal_signer()?;
                if sender != seal_sender {
                    return Err(anyhow!(
                        "Commit seal signer is different from message signer"
                    ));
                }

                self.messages.add_commit_message(commit, sender).await;
            }
            IBFTMessage::RoundChange(round_change) => {
                let view = round_change.view();

                if view.height <= self.height() {
                    return Ok(());
                }

                let sender = round_change.recover_signer()?;
                if !self.validator_manager.is_validator(sender, view.height) {
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
    proposal_messages: Arc<Mutex<ViewMap<Arc<ProposalMessageSigned>>>>,
    prepare_messages: Arc<Mutex<ViewSenderMap<Arc<PrepareMessageSigned>>>>,
    commit_messages: Arc<Mutex<ViewSenderMap<Arc<CommitMessageSigned>>>>,
    round_change_messages: Arc<Mutex<ViewSenderMap<Arc<RoundChangeMessageSigned>>>>,

    proposal_tx: broadcast::Sender<Arc<ProposalMessageSigned>>,
    prepare_tx: broadcast::Sender<Arc<PrepareMessageSigned>>,
    commit_tx: broadcast::Sender<Arc<CommitMessageSigned>>,
    round_change_tx: broadcast::Sender<Arc<RoundChangeMessageSigned>>,
}

impl ConsensusMessages {
    pub fn new() -> Self {
        Self {
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
    pub async fn add_proposal_message(&self, proposal: Arc<ProposalMessageSigned>) {
        let mut proposal_messages = self.proposal_messages.lock().await;
        let entry = proposal_messages.view_entry(proposal.view());
        if let btree_map::Entry::Vacant(entry) = entry {
            entry.insert(proposal.clone());

            let _ = self.proposal_tx.send(proposal);
        }
    }

    /// Adds a prepare message if not already exists for the view and sender, and broadcasts it.
    pub async fn add_prepare_message(&self, prepare: Arc<PrepareMessageSigned>, sender: Address) {
        let mut prepare_messages = self.prepare_messages.lock().await;
        let entry = prepare_messages.sender_entry(prepare.view(), sender);
        if let hash_map::Entry::Vacant(entry) = entry {
            entry.insert(prepare.clone());

            let _ = self.prepare_tx.send(prepare);
        }
    }

    /// Adds a commit message if not already exists for the view and sender, and broadcasts it.
    pub async fn add_commit_message(&self, commit: Arc<CommitMessageSigned>, sender: Address) {
        let mut commit_messages = self.commit_messages.lock().await;
        let entry = commit_messages.sender_entry(commit.view(), sender);
        if let hash_map::Entry::Vacant(entry) = entry {
            entry.insert(commit.clone());

            let _ = self.commit_tx.send(commit);
        }
    }

    /// Adds a round change message if not already exists for the view and sender, and broadcasts it.
    pub async fn add_round_change_message(
        &self,
        round_change: Arc<RoundChangeMessageSigned>,
        sender: Address,
    ) {
        let mut round_change_messages = self.round_change_messages.lock().await;
        let entry = round_change_messages.sender_entry(round_change.view(), sender);
        if let hash_map::Entry::Vacant(entry) = entry {
            entry.insert(round_change.clone());

            let _ = self.round_change_tx.send(round_change);
        }
    }

    pub fn subscribe_proposal(&self) -> broadcast::Receiver<Arc<ProposalMessageSigned>> {
        self.proposal_tx.subscribe()
    }

    pub fn subscribe_prepare(&self) -> broadcast::Receiver<Arc<PrepareMessageSigned>> {
        self.prepare_tx.subscribe()
    }

    pub fn subscribe_commit(&self) -> broadcast::Receiver<Arc<CommitMessageSigned>> {
        self.commit_tx.subscribe()
    }

    pub fn subscribe_round_change(&self) -> broadcast::Receiver<Arc<RoundChangeMessageSigned>> {
        self.round_change_tx.subscribe()
    }

    pub async fn get_valid_prepare_message_count<F>(&self, view: View, verify_fn: F) -> usize
    where
        F: Fn(&PrepareMessageSigned) -> bool,
    {
        let mut prepare_messages = self.prepare_messages.lock().await;
        let messages = prepare_messages.view_entry(view).or_default();

        // Prune invalid messages
        messages.retain(|_, prepare| verify_fn(prepare));

        messages.len()
    }

    pub async fn get_valid_commit_message_seals<F>(
        &self,
        view: View,
        verify_fn: F,
    ) -> Vec<PrimitiveSignature>
    where
        F: Fn(&CommitMessageSigned) -> bool,
    {
        let mut commit_messages = self.commit_messages.lock().await;
        let messages = commit_messages.view_entry(view).or_default();

        // Prune invalid messages
        messages.retain(|_, commit| verify_fn(commit));

        messages
            .values()
            .map(|commit| commit.commit_seal())
            .collect()
    }

    pub async fn get_valid_round_change_messages<F>(
        &self,
        view: View,
        verify_fn: F,
    ) -> Vec<Arc<RoundChangeMessageSigned>>
    where
        F: Fn(&RoundChangeMessageSigned) -> bool,
    {
        let mut round_change_messages = self.round_change_messages.lock().await;
        let messages = round_change_messages.view_entry(view).or_default();

        // Prune invalid messages
        messages.retain(|_, round_change| verify_fn(round_change));

        messages.values().cloned().collect()
    }

    pub async fn get_proposal_message(&self, view: View) -> Option<Arc<ProposalMessageSigned>> {
        let proposal_messages = self.proposal_messages.lock().await;
        proposal_messages.get_by_view(view).cloned()
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
struct ViewMap<T>(BTreeMap<u64, BTreeMap<u32, T>>);

impl<T> ViewMap<T> {
    fn new() -> Self {
        Self(Default::default())
    }

    // Prune messages less than height
    fn prune(&mut self, height: u64) {
        self.0 = self.0.split_off(&height);
    }

    fn view_entry(&mut self, view: View) -> btree_map::Entry<'_, u32, T> {
        self.0.entry(view.height).or_default().entry(view.round)
    }

    fn get_by_view(&self, view: View) -> Option<&T> {
        self.0
            .get(&view.height)
            .and_then(|btree| btree.get(&view.round))
    }
}
