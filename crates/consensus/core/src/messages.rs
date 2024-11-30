use std::collections::{btree_map, hash_map, BTreeMap, HashMap};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use tokio::sync::broadcast;
use tonic_primitives::Address;

use super::types::{
    CommitMessageSigned, PrepareMessageSigned, ProposalMessageSigned, RoundChangeMessageSigned,
    View,
};

const CHANNEL_SIZE: usize = 128;

/// Container for consensus messages received by peers. `ConsensusMessages` ensures that:
/// - no duplicate messages from the same sender for the same view; only the first is accepted, and the rest are discarded.
///
/// Certain checks must be done before adding a message:
/// - message has a valid signature
/// - message signer is a valid validator for the corresponding height
/// - if proposal, signer must be the proposer for the corresponding height and round
///
/// Also provides subscription capabilities.
#[derive(Debug)]
pub struct ConsensusMessages {
    proposal_messages: ViewMap<Rc<ProposalMessageSigned>>,
    prepare_messages: ViewSenderMap<Rc<PrepareMessageSigned>>,
    commit_messages: ViewSenderMap<Rc<CommitMessageSigned>>,
    round_change_messages: ViewSenderMap<Rc<RoundChangeMessageSigned>>,

    proposal_tx: broadcast::Sender<Rc<ProposalMessageSigned>>,
    prepare_tx: broadcast::Sender<Rc<PrepareMessageSigned>>,
    commit_tx: broadcast::Sender<Rc<CommitMessageSigned>>,
    round_change_tx: broadcast::Sender<Rc<RoundChangeMessageSigned>>,
}

impl ConsensusMessages {
    pub fn new() -> Self {
        Self {
            proposal_messages: ViewMap::new(),
            prepare_messages: ViewSenderMap::new(),
            commit_messages: ViewSenderMap::new(),
            round_change_messages: ViewSenderMap::new(),
            proposal_tx: broadcast::channel(CHANNEL_SIZE).0,
            prepare_tx: broadcast::channel(CHANNEL_SIZE).0,
            commit_tx: broadcast::channel(CHANNEL_SIZE).0,
            round_change_tx: broadcast::channel(CHANNEL_SIZE).0,
        }
    }

    /// Prunes messages less than height
    pub async fn prune(&mut self, height: u64) {
        self.proposal_messages.prune(height);
        self.prepare_messages.prune(height);
        self.commit_messages.prune(height);
        self.round_change_messages.prune(height);
    }

    /// Adds a proposal message if not already exists for the view, and broadcasts it.
    pub async fn add_proposal_message(&mut self, proposal: ProposalMessageSigned) {
        let entry = self.proposal_messages.view_entry(proposal.view);
        if let btree_map::Entry::Vacant(entry) = entry {
            let proposal = Rc::new(proposal);

            entry.insert(proposal.clone());

            let _ = self.proposal_tx.send(proposal);
        }
    }

    /// Adds a prepare message if not already exists for the view and sender, and broadcasts it.
    pub async fn add_prepare_message(&mut self, prepare: PrepareMessageSigned, sender: Address) {
        let entry = self.prepare_messages.sender_entry(prepare.view, sender);
        if let hash_map::Entry::Vacant(entry) = entry {
            let prepare = Rc::new(prepare);

            entry.insert(prepare.clone());

            let _ = self.prepare_tx.send(prepare);
        }
    }

    /// Adds a commit message if not already exists for the view and sender, and broadcasts it.
    pub async fn add_commit_message(&mut self, commit: CommitMessageSigned, sender: Address) {
        let entry = self.commit_messages.sender_entry(commit.view, sender);
        if let hash_map::Entry::Vacant(entry) = entry {
            let commit = Rc::new(commit);

            entry.insert(commit.clone());

            let _ = self.commit_tx.send(commit);
        }
    }

    pub async fn add_round_change_message(
        &mut self,
        round_change: RoundChangeMessageSigned,
        sender: Address,
    ) {
        let entry = self
            .round_change_messages
            .sender_entry(round_change.view, sender);
        if let hash_map::Entry::Vacant(entry) = entry {
            let round_change = Rc::new(round_change);

            entry.insert(round_change.clone());

            let _ = self.round_change_tx.send(round_change);
        }
    }

    pub fn subscribe_proposal(&self) -> broadcast::Receiver<Rc<ProposalMessageSigned>> {
        self.proposal_tx.subscribe()
    }

    pub fn subscribe_prepare(&self) -> broadcast::Receiver<Rc<PrepareMessageSigned>> {
        self.prepare_tx.subscribe()
    }

    pub fn subscribe_commit(&self) -> broadcast::Receiver<Rc<CommitMessageSigned>> {
        self.commit_tx.subscribe()
    }

    pub fn subscribe_round_change(&self) -> broadcast::Receiver<Rc<RoundChangeMessageSigned>> {
        self.round_change_tx.subscribe()
    }

    pub async fn get_valid_round_change_messages<F>(
        &mut self,
        view: View,
        validate_fn: F,
    ) -> Vec<Rc<RoundChangeMessageSigned>>
    where
        F: Fn(&RoundChangeMessageSigned) -> bool,
    {
        let messages = self.round_change_messages.view_entry(view).or_default();

        // Prune invalid messages
        messages.retain(|_, round_change| validate_fn(round_change));

        messages.values().cloned().collect()
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
}
