use crate::{pending::PendingPool, PoolTransaction};

pub struct TransactionPool {
    pending: PendingPool,
}

impl TransactionPool {
    pub fn add_transaction(&mut self, transaction: PoolTransaction) {
        // TODO: verify tx

        // let account = self.storage.get_account(transaction.from)
        // if account.nonce == transaction.nonce {
        //   self.pending.add_transaction()
        // }
    }
}

// 1. add_transaction()
// 2. validate tx -> validate signature
// 3. db.get_account().nonce
