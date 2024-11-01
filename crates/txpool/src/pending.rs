use std::cmp::{self, Ordering};
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use tonic_primitives::address::Address;

use crate::{PoolTransaction, TransactionId};

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

    pub fn add_transaction(&mut self, transaction: Arc<PoolTransaction>, base_fee: u64) {
        let submission_id = self.next_id();
        let priority = self.priority_by_tip(&transaction, base_fee);

        let pending_tx = PendingTransaction::new(submission_id, transaction, priority);

        match self.by_id.entry(pending_tx.id()) {
            Entry::Occupied(mut entry) => {
                // If transaction id exists, this is a replacement transaction,
                // and replacement transactions are only accepted if the tip
                // is higher than the previous transaction.
                let existing_tx = entry.get_mut();
                if pending_tx.cmp(existing_tx) == Ordering::Greater {
                    assert!(self.all.remove(existing_tx));
                    assert!(self.all.insert(pending_tx.clone()));
                    *existing_tx = pending_tx;
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(pending_tx.clone());
                assert!(self.all.insert(pending_tx));
            }
        };
    }

    pub fn best_iter(&self) -> impl Iterator<Item = &Arc<PoolTransaction>> + '_ {
        self.all.iter().map(|tx| &tx.transaction).rev()
    }

    fn priority_by_tip(&self, transaction: &PoolTransaction, base_fee: u64) -> u64 {
        let transaction = &transaction.transaction;
        assert!(
            transaction.max_fee_per_gas >= base_fee,
            "Pooled transaction must have max_fee_per_gas >= base_fee"
        );

        cmp::min(
            transaction.max_fee_per_gas - base_fee,
            transaction.max_priority_fee_per_gas,
        )
    }

    fn next_id(&mut self) -> u64 {
        let id = self.submission_id;
        self.submission_id = self.submission_id.wrapping_add(1);
        id
    }
}

#[derive(Clone, Debug)]
struct PendingTransaction {
    submission_id: u64,
    /// Inner `PooledTransaction`.
    transaction: Arc<PoolTransaction>,
    /// Priority of the pending transaction. The higher the better.
    priority: u64,
}

impl PendingTransaction {
    fn new(submission_id: u64, transaction: Arc<PoolTransaction>, priority: u64) -> Self {
        Self {
            submission_id,
            transaction,
            priority,
        }
    }

    fn id(&self) -> TransactionId {
        self.transaction.id()
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

    use crate::{PoolTransaction, TransactionId};

    use super::PendingPool;

    fn generate_pool_tx(
        from: Address,
        nonce: u64,
        max_fee_per_gas: u64,
        max_priority_fee_per_gas: u64,
    ) -> PoolTransaction {
        PoolTransaction {
            transaction: Transaction {
                from,
                to: TransactionKind::CreateToken,
                nonce,
                value: Default::default(),
                gas_limit: 5_000_000,
                max_fee_per_gas,
                max_priority_fee_per_gas,
                chain_id: ChainId::Testnet,
            },
            transaction_id: TransactionId::new(from, nonce),
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

        let v = pool.best_iter().map(|tx| {
            (tx.transaction.nonce, tx.transaction.max_priority_fee_per_gas)
        }).collect::<Vec<_>>();

        dbg!(v);
    }
}
