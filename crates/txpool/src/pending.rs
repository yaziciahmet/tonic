use std::cmp::{self, Ordering};
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::pool_transaction::{PoolTransaction, TransactionId};

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

    pub fn add_transaction(&mut self, tx: Arc<PoolTransaction>, base_fee: u64) {
        let submission_id = self.next_submission_id();
        let priority = self.priority_by_tip(&tx, base_fee);

        let pending_tx = PendingTransaction::new(submission_id, tx, priority);

        match self.tx_by_id.entry(pending_tx.id()) {
            Entry::Occupied(mut entry) => {
                // If transaction id exists, this is a replacement transaction,
                // and replacement transactions are only accepted if the tip
                // is higher than the previous transaction.
                let existing_tx = entry.get_mut();
                if pending_tx.cmp(existing_tx) == Ordering::Greater {
                    assert!(self.all_txs.remove(existing_tx));
                    assert!(self.all_txs.insert(pending_tx.clone()));
                    *existing_tx = pending_tx;
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(pending_tx.clone());
                assert!(self.all_txs.insert(pending_tx));
            }
        };
    }

    pub fn best_iter(&self) -> impl Iterator<Item = &Arc<PoolTransaction>> + '_ {
        self.all_txs.iter().map(|tx| &tx.tx).rev()
    }

    fn priority_by_tip(&self, tx: &PoolTransaction, base_fee: u64) -> u64 {
        let tx = &tx.tx;
        assert!(
            tx.max_fee_per_gas >= base_fee,
            "Pooled transaction must have max_fee_per_gas >= base_fee"
        );

        cmp::min(tx.max_fee_per_gas - base_fee, tx.max_priority_fee_per_gas)
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
    tx: Arc<PoolTransaction>,
    /// Priority of the pending transaction. The higher the better.
    priority: u64,
}

impl PendingTransaction {
    fn new(submission_id: u64, tx: Arc<PoolTransaction>, priority: u64) -> Self {
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

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Instant};

    use tonic_primitives::{
        address::Address,
        chain_id::ChainId,
        transaction::{Transaction, TransactionKind},
    };

    use crate::pool_transaction::{PoolTransaction, TransactionId};

    use super::PendingPool;

    fn generate_pool_tx(
        from: Address,
        nonce: u64,
        max_fee_per_gas: u64,
        max_priority_fee_per_gas: u64,
    ) -> PoolTransaction {
        PoolTransaction {
            tx: Transaction {
                from,
                to: TransactionKind::CreateToken,
                nonce,
                value: Default::default(),
                gas_limit: 5_000_000,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                chain_id: ChainId::Testnet,
            },
            tx_id: TransactionId::new(from, nonce),
            timestamp: Instant::now(),
        }
    }

    #[test]
    fn add_transaction() {
        let mut pool = PendingPool::new();

        let address = Address::random();
        let tx = generate_pool_tx(address, 0, 10, 3);
        pool.add_transaction(Arc::new(tx), 2);

        let tx = generate_pool_tx(address, 1, 10, 5);
        pool.add_transaction(Arc::new(tx), 2);

        let tx = generate_pool_tx(address, 2, 10, 4);
        pool.add_transaction(Arc::new(tx), 2);

        let tx = generate_pool_tx(address, 3, 10, 9);
        pool.add_transaction(Arc::new(tx), 2);

        // replace
        let tx = generate_pool_tx(address, 2, 10, 7);
        pool.add_transaction(Arc::new(tx), 2);

        let v = pool
            .best_iter()
            .map(|tx| (tx.tx.nonce, tx.tx.max_priority_fee_per_gas))
            .collect::<Vec<_>>();

        dbg!(v);

        let v = pool
            .all_txs
            .iter()
            .map(|tx| (tx.tx.tx_id.sender.to_string(), tx.tx.tx_id.nonce))
            .collect::<Vec<_>>();
        dbg!(v);
    }
}
