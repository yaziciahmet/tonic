use std::cmp::{self, Ordering};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use tonic_primitives::{Transaction, TransactionT};

use crate::transaction::{PooledTransaction, TransactionId};

pub struct PendingPool {
    submission_id: u64,

    by_id: BTreeMap<TransactionId, PendingTransaction>,
    all: BTreeSet<PendingTransaction>,
}

impl PendingPool {
    pub fn new() -> Self {
        Self {
            submission_id: 0,
            by_id: BTreeMap::new(),
            all: BTreeSet::new(),
        }
    }

    pub fn add_transaction(&mut self, tx: Arc<PooledTransaction>, base_fee: u128) {
        assert!(
            !self.contains(&tx.id()),
            "Transaction already exists.\nexisting = {:?}\nincoming = {:?}",
            self.get(&tx.id()).unwrap(),
            tx,
        );

        let submission_id = self.next_submission_id();
        let priority = self.priority_by_tip(&tx, base_fee);

        let pending_tx = PendingTransaction::new(submission_id, tx, priority);

        self.by_id.insert(pending_tx.id(), pending_tx.clone());
        self.all.insert(pending_tx);
    }

    pub fn remove_transaction(&mut self, tx_id: &TransactionId) {
        let removed_tx = self
            .by_id
            .remove(tx_id)
            .unwrap_or_else(|| panic!("Transaction does not exist {:?}", tx_id));
        self.all.remove(&removed_tx);
    }

    fn priority_by_tip(&self, tx: &PooledTransaction, base_fee: u128) -> u128 {
        assert!(
            tx.max_fee_per_gas() >= base_fee,
            "Pooled transaction must have max_fee_per_gas >= base_fee to get into PendingPool"
        );

        match &tx.transaction {
            Transaction::Legacy(tx) => tx.gas_price - base_fee,
            Transaction::Eip1559(tx) => {
                cmp::min(tx.max_fee_per_gas - base_fee, tx.max_priority_fee_per_gas)
            }
            tx => panic!("{} is not supported", tx.tx_type()),
        }
    }

    fn next_submission_id(&mut self) -> u64 {
        let id = self.submission_id;
        self.submission_id = self.submission_id.wrapping_add(1);
        id
    }

    fn contains(&self, tx_id: &TransactionId) -> bool {
        self.by_id.contains_key(tx_id)
    }

    fn get(&self, tx_id: &TransactionId) -> Option<&PendingTransaction> {
        self.by_id.get(tx_id)
    }
}

#[derive(Clone, Debug)]
struct PendingTransaction {
    submission_id: u64,
    /// Inner `PooledTransaction`.
    tx: Arc<PooledTransaction>,
    /// Priority of the pending transaction. The higher the better.
    priority: u128,
}

impl PendingTransaction {
    fn new(submission_id: u64, tx: Arc<PooledTransaction>, priority: u128) -> Self {
        Self {
            submission_id,
            tx,
            priority,
        }
    }

    fn id(&self) -> TransactionId {
        self.tx.id()
    }
}

impl Eq for PendingTransaction {}

impl PartialEq for PendingTransaction {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl PartialOrd for PendingTransaction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PendingTransaction {
    fn cmp(&self, other: &Self) -> Ordering {
        // This compares by `priority` and only if two tx have the exact same priority this compares
        // the unique `submission_id`. This ensures that transactions with same priority are not
        // equal, so they're not replaced in the set
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.submission_id.cmp(&self.submission_id))
    }
}
