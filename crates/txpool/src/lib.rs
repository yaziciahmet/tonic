mod pending;
mod queued;
mod txpool;

use std::cmp::Ordering;
use std::time::Instant;

use tonic_primitives::address::Address;
use tonic_primitives::transaction::Transaction;

#[derive(Debug, Copy, Clone)]
pub struct TransactionId {
    pub sender: Address,
    pub nonce: u64,
}

impl TransactionId {
    pub fn new(sender: Address, nonce: u64) -> Self {
        Self { sender, nonce }
    }
}

impl Eq for TransactionId {}

impl PartialEq for TransactionId {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl PartialOrd for TransactionId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TransactionId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sender
            .cmp(&other.sender)
            .then_with(|| self.nonce.cmp(&other.nonce))
    }
}

#[derive(Debug)]
pub struct PoolTransaction {
    pub transaction: Transaction,
    pub transaction_id: TransactionId,
    pub timestamp: Instant,
}

impl PoolTransaction {
    pub fn id(&self) -> TransactionId {
        self.transaction_id
    }
}
