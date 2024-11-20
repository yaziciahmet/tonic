use std::time::Instant;
use std::{cmp::Ordering, ops::Deref};

use tonic_primitives::{Address, Transaction, TransactionSignedEcRecovered, TransactionT, U256};

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
pub struct PooledTransaction {
    tx: TransactionSignedEcRecovered,
    tx_id: TransactionId,
    timestamp: Instant,
    cost: U256,
}

impl From<TransactionSignedEcRecovered> for PooledTransaction {
    fn from(tx: TransactionSignedEcRecovered) -> Self {
        let tx_id = TransactionId::new(tx.signer(), tx.nonce());

        let gas_cost = match &tx.transaction {
            Transaction::Legacy(t) => {
                U256::from(t.gas_price).saturating_mul(U256::from(t.gas_limit))
            }
            Transaction::Eip1559(t) => {
                U256::from(t.max_fee_per_gas).saturating_mul(U256::from(t.gas_limit))
            }
            tx => panic!(
                "{} is not supported, and must already be checked before arriving to mempool",
                tx.tx_type()
            ),
        };
        let mut cost = tx.value();
        cost = cost.saturating_add(gas_cost);

        Self {
            tx,
            tx_id,
            timestamp: Instant::now(),
            cost,
        }
    }
}

impl Deref for PooledTransaction {
    type Target = TransactionSignedEcRecovered;

    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

impl PooledTransaction {
    pub fn id(&self) -> TransactionId {
        self.tx_id
    }

    pub fn timestamp(&self) -> Instant {
        self.timestamp
    }

    pub fn cost(&self) -> U256 {
        self.cost
    }

    pub fn is_underpriced(
        &self,
        replacement: &PooledTransaction,
        price_bump_percentage: u128,
    ) -> bool {
        if replacement.max_fee_per_gas()
            < self.max_fee_per_gas() * (100 + price_bump_percentage) / 100
        {
            return true;
        }

        if let (Some(self_priority), Some(replacement_priority)) = (
            self.max_priority_fee_per_gas(),
            replacement.max_priority_fee_per_gas(),
        ) {
            if replacement_priority < self_priority * (100 + price_bump_percentage) / 100 {
                return true;
            }
        }

        false
    }
}
