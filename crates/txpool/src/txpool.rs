use std::collections::BTreeMap;
use std::sync::Arc;

use crate::pending::PendingPool;
use crate::pool_transaction::{PoolTransaction, TransactionId};

pub struct TransactionPool {
    pending: PendingPool,
}

impl TransactionPool {
    pub fn add_transaction(&mut self, tx: PoolTransaction) {
        // TODO: verify tx

        // let account = self.storage.get_account(transaction.from)
        // if account.nonce == transaction.nonce {
        //   self.pending.add_transaction()
        // }
    }
}

pub struct AllTransactions {
    txs: BTreeMap<TransactionId, PoolTransactionWithSubpool>,
}

impl AllTransactions {
    pub fn new() -> Self {
        Self {
            txs: BTreeMap::default(),
        }
    }

    pub fn add_transaction(&mut self, tx: PoolTransaction) {
        let tx = Arc::new(tx);
    }
}

#[derive(Clone, Debug)]
#[repr(u8)]
enum Subpool {
    Pending = 0,
    Queued = 1,
}

struct PoolTransactionWithSubpool {
    tx: Arc<PoolTransaction>,
    subpool: Subpool,
}

enum AddTransactionResult {
    Ok(AddTransactionOk),
    Err(AddTransactionErr),
}

struct AddTransactionOk {
    tx: Arc<PoolTransaction>,
    replaced_tx: Option<(Arc<PoolTransaction>, Subpool)>,
    move_to: Subpool,
}

enum AddTransactionErr {}
