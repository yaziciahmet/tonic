use async_trait::async_trait;
use madsim::buggify::buggify_with_prob;
use tokio::sync::mpsc;
use tonic_primitives::Address;

use crate::backend::{BlockBuilder, BlockVerifier, Broadcast, ValidatorManager};
use crate::types::{FinalizedBlock, IBFTBroadcastMessage, IBFTReceivedMessage, View};

#[derive(Clone)]
pub struct Mock {
    validators: Vec<MockValidator>,
}

impl Mock {
    pub fn new(mut validators: Vec<MockValidator>) -> Self {
        assert!(!validators.is_empty());
        validators.sort_by_key(|v| v.address);
        Self { validators }
    }
}

impl ValidatorManager for Mock {
    fn is_proposer(&self, address: Address, view: View) -> bool {
        let idx = (view.height as usize + view.round as usize) % self.validators.len();
        self.validators[idx].address == address
    }

    fn is_validator(&self, address: Address, _: u64) -> bool {
        self.validators
            .iter()
            .find(|v| v.address == address)
            .is_some()
    }

    fn quorum(&self, _: u64) -> usize {
        (self.validators.len() * 2 / 3) + 1
    }
}

#[async_trait]
impl Broadcast for Mock {
    async fn broadcast_message<'a>(&self, message: IBFTBroadcastMessage<'a>) {
        let sender = match message {
            IBFTBroadcastMessage::Proposal(proposal) => proposal.recover_signer(),
            IBFTBroadcastMessage::Prepare(prepare) => prepare.recover_signer(),
            IBFTBroadcastMessage::Commit(commit) => commit.recover_signer(),
            IBFTBroadcastMessage::RoundChange(round_change) => round_change.recover_signer(),
        }
        .unwrap();

        let serialized = borsh::to_vec(&message).unwrap();
        for validator in &self.validators {
            if validator.address != sender {
                // Both have the same message structure
                let mut new_message: IBFTReceivedMessage = borsh::from_slice(&serialized).unwrap();
                if buggify_with_prob(0.05) {
                    new_message.buggify_sig();
                }
                validator.p2p_tx.send(new_message).await.unwrap();
            }
        }
    }

    async fn broadcast_block(&self, _: &FinalizedBlock) {}
}

impl BlockVerifier for Mock {
    type Error = &'static str;

    fn verify_block(&self, raw_block: &[u8]) -> Result<(), Self::Error> {
        if raw_block == &[1, 2, 3] {
            Ok(())
        } else {
            Err("Invalid block")
        }
    }
}

impl BlockBuilder for Mock {
    type Error = &'static str;

    fn build_block(&self, _: u64) -> Result<Vec<u8>, Self::Error> {
        if buggify_with_prob(0.05) {
            if buggify_with_prob(0.2) {
                Err("Could not build a block")
            } else {
                Ok(vec![1, 2])
            }
        } else {
            Ok(vec![1, 2, 3])
        }
    }
}

#[derive(Clone)]
pub struct MockValidator {
    address: Address,
    p2p_tx: mpsc::Sender<IBFTReceivedMessage>,
}

impl MockValidator {
    pub fn new(address: Address, p2p_tx: mpsc::Sender<IBFTReceivedMessage>) -> Self {
        Self { address, p2p_tx }
    }
}
