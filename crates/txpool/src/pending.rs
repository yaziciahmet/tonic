use std::cmp::{self, Ordering};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use tonic_primitives::{Transaction, TransactionT};

use crate::transaction::{PooledTransaction, TransactionId};

pub struct PendingPool {
    submission_id: u64,

    tx_by_id: BTreeMap<TransactionId, PendingTransaction>,
    all_txs: BTreeSet<PendingTransaction>,
}

impl PendingPool {
    pub fn new() -> Self {
        Self {
            submission_id: 0,
            tx_by_id: BTreeMap::new(),
            all_txs: BTreeSet::new(),
        }
    }

    pub fn add_transaction(&mut self, tx: Arc<PooledTransaction>, base_fee: u128) {
        let submission_id = self.next_submission_id();
        let priority = self.priority_by_tip(&tx, base_fee);

        let pending_tx = PendingTransaction::new(submission_id, tx, priority);

        assert_eq!(
            self.tx_by_id.insert(pending_tx.id(), pending_tx.clone()),
            None,
            "PendingPool.tx_by_id should not receive duplicate tx request"
        );
        assert!(
            self.all_txs.insert(pending_tx),
            "PendingPool.all_txs should not receive duplicate tx request"
        );
    }

    pub fn remove_transaction(&mut self, tx_id: &TransactionId) {
        let removed_tx = self.tx_by_id.remove(tx_id).expect(
            "PendingPool.tx_by_id should not receive remove tx request for non-existing tx",
        );
        assert!(
            self.all_txs.remove(&removed_tx),
            "PendingPool.all_txs should not receive remove tx request for non-existing tx",
        );
    }

    pub fn _best_iter(&self) -> impl Iterator<Item = &Arc<PooledTransaction>> + '_ {
        self.all_txs.iter().map(|tx| &tx.tx).rev()
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
