use std::collections::{BTreeMap, HashMap};
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
pub struct ConsensusMessages {
    /// map[height][round][sender] => messages
    by_height: BTreeMap<u64, BTreeMap<u32, HashMap<Address, SenderMessages>>>,

    proposal_tx: broadcast::Sender<Rc<ProposalMessageSigned>>,
    prepare_tx: broadcast::Sender<Rc<PrepareMessageSigned>>,
    commit_tx: broadcast::Sender<Rc<CommitMessageSigned>>,
    round_change_tx: broadcast::Sender<Rc<RoundChangeMessageSigned>>,
}

impl ConsensusMessages {
    pub fn new() -> Self {
        Self {
            by_height: BTreeMap::new(),
            proposal_tx: broadcast::channel(CHANNEL_SIZE).0,
            prepare_tx: broadcast::channel(CHANNEL_SIZE).0,
            commit_tx: broadcast::channel(CHANNEL_SIZE).0,
            round_change_tx: broadcast::channel(CHANNEL_SIZE).0,
        }
    }

    /// Prunes messages less than height
    pub async fn prune(&mut self, height: u64) {
        self.by_height = self.by_height.split_off(&height);
    }

    pub async fn add_proposal_message(&mut self, proposal: ProposalMessageSigned, sender: Address) {
        let proposal = Rc::new(proposal);

        let messages = self.get_sender_messages(proposal.view, sender);
        // return early if sender already sent a proposal for this view
        if messages.proposal.is_some() {
            return;
        }

        messages.proposal = Some(proposal.clone());

        // only errors if there are no receivers
        let _ = self.proposal_tx.send(proposal);
    }

    pub async fn add_prepare_message(&mut self, prepare: PrepareMessageSigned, sender: Address) {
        let prepare = Rc::new(prepare);

        let messages = self.get_sender_messages(prepare.view, sender);
        // return early if sender already sent a prepare for this view
        if messages.prepare.is_some() {
            return;
        }

        messages.prepare = Some(prepare.clone());

        // only errors if there are no receivers
        let _ = self.prepare_tx.send(prepare);
    }

    pub async fn add_commit_message(&mut self, commit: CommitMessageSigned, sender: Address) {
        let commit = Rc::new(commit);

        let messages = self.get_sender_messages(commit.view, sender);
        // return early if sender already sent a commit for this view
        if messages.commit.is_some() {
            return;
        }

        messages.commit = Some(commit.clone());

        // only errors if there are no receivers
        let _ = self.commit_tx.send(commit);
    }

    pub async fn add_round_change_message(
        &mut self,
        round_change: RoundChangeMessageSigned,
        sender: Address,
    ) {
        let round_change = Rc::new(round_change);

        let messages = self.get_sender_messages(round_change.view, sender);
        // return early if sender already sent a round change for this view
        if messages.round_change.is_some() {
            return;
        }

        messages.round_change = Some(round_change.clone());

        // only errors if there are no receivers
        let _ = self.round_change_tx.send(round_change);
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
        let by_sender = self.get_view_messages(view);

        let mut valid_messages = vec![];
        for (_, messages) in by_sender.iter_mut() {
            let Some(round_change) = messages.round_change.as_ref() else {
                continue;
            };

            if validate_fn(round_change) {
                valid_messages.push(round_change.clone());
            } else {
                // prune if invalid
                messages.round_change = None;
            }
        }

        valid_messages
    }

    fn get_sender_messages(&mut self, view: View, sender: Address) -> &mut SenderMessages {
        self.get_view_messages(view).entry(sender).or_default()
    }

    fn get_view_messages(&mut self, view: View) -> &mut HashMap<Address, SenderMessages> {
        self.by_height
            .entry(view.height)
            .or_insert_with(BTreeMap::new)
            .entry(view.round)
            .or_insert_with(HashMap::new)
    }
}

#[derive(Debug, Default)]
struct SenderMessages {
    proposal: Option<Rc<ProposalMessageSigned>>,
    prepare: Option<Rc<PrepareMessageSigned>>,
    commit: Option<Rc<CommitMessageSigned>>,
    round_change: Option<Rc<RoundChangeMessageSigned>>,
}
