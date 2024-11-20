use std::sync::Arc;

use crate::transaction::{PooledTransaction, TransactionId};

pub struct ParkedPool {}

impl ParkedPool {
    pub fn new() -> Self {
        Self {}
    }

    pub fn add_transaction(&mut self, _tx: Arc<PooledTransaction>) {
        todo!()
    }

    pub fn remove_transaction(&mut self, _tx_id: &TransactionId) {
        todo!()
    }
}
