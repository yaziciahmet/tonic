use std::cmp::Ordering;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use tonic_primitives::{Address, TransactionT, U256};

use crate::config::Config;
use crate::parked::ParkedPool;
use crate::pending::PendingPool;
use crate::transaction::{PooledTransaction, TransactionId};

pub struct TransactionPool {
    // Transactions are ready to be included in the block
    pending_pool: PendingPool,
    // 1. account balance is not enough
    // 2. account nonce has gaps
    queued_pool: ParkedPool,
    // Dynamic basefee conditions are not met, but might be met in the future
    basefee_pool: ParkedPool,
    // All transactions
    all_txs: AllTransactions,
    // Account information
    account_info: HashMap<Address, AccountInfo>,
}

impl TransactionPool {
    pub fn new(config: Config) -> Self {
        Self {
            pending_pool: PendingPool::new(),
            queued_pool: ParkedPool::new(),
            basefee_pool: ParkedPool::new(),
            all_txs: AllTransactions::new(config),
            account_info: HashMap::default(),
        }
    }

    pub fn add_transaction(
        &mut self,
        tx: PooledTransaction,
        on_chain_nonce: u64,
        on_chain_balance: U256,
    ) -> Result<(), TransactionPoolError> {
        self.account_info
            .entry(tx.signer())
            .or_default()
            .update(on_chain_nonce, on_chain_balance);

        let AddTransactionOk {
            tx,
            subpool,
            replaced_tx,
        } = self
            .all_txs
            .add_transaction(tx, on_chain_nonce, on_chain_balance)?;

        if let Some((replaced_tx, replaced_subpool)) = replaced_tx {
            self.remove_transaction_from_subpool(replaced_tx.id(), replaced_subpool);
        }

        self.add_transaction_to_subpool(tx, subpool);

        Ok(())
    }

    fn add_transaction_to_subpool(&mut self, tx: Arc<PooledTransaction>, subpool: Subpool) {
        match subpool {
            Subpool::Pending => self.pending_pool.add_transaction(tx, self.all_txs.basefee),
            Subpool::Queued => self.queued_pool.add_transaction(tx),
            Subpool::Basefee => self.basefee_pool.add_transaction(tx),
        }
    }

    fn remove_transaction_from_subpool(&mut self, tx_id: TransactionId, subpool: Subpool) {
        match subpool {
            Subpool::Pending => self.pending_pool.remove_transaction(&tx_id),
            Subpool::Queued => self.queued_pool.remove_transaction(&tx_id),
            Subpool::Basefee => self.basefee_pool.remove_transaction(&tx_id),
        }
    }
}

struct AllTransactions {
    config: Config,
    txs: BTreeMap<TransactionId, (Arc<PooledTransaction>, Subpool)>,
    // TODO: handle this better
    basefee: u128,
}

impl AllTransactions {
    fn new(config: Config) -> Self {
        Self {
            config,
            txs: BTreeMap::default(),
            basefee: 0,
        }
    }

    fn add_transaction(
        &mut self,
        tx: PooledTransaction,
        on_chain_nonce: u64,
        on_chain_balance: U256,
    ) -> Result<AddTransactionOk, AddTransactionError> {
        self.validate_tx(&tx)?;

        let subpool = match tx.nonce().cmp(&on_chain_nonce) {
            // Has next nonce
            Ordering::Equal => {
                // Has enough balance
                if on_chain_balance >= tx.cost() {
                    // Has higher max_fee_per_gas
                    if tx.max_fee_per_gas() >= self.basefee {
                        Subpool::Pending
                    } else {
                        Subpool::Basefee
                    }
                } else {
                    Subpool::Queued
                }
            }
            Ordering::Greater => Subpool::Queued,
            Ordering::Less => {
                return Err(AddTransactionError::NonceTooLow);
            }
        };

        let tx = Arc::new(tx);

        let replaced_tx = match self.txs.entry(tx.id()) {
            Entry::Vacant(entry) => {
                entry.insert((tx.clone(), subpool));
                None
            }
            Entry::Occupied(mut entry) => {
                let existing_tx = entry.get_mut();
                if existing_tx
                    .0
                    .is_underpriced(&tx, self.config.price_bump_percentage)
                {
                    return Err(AddTransactionError::ReplacementTxUnderpriced);
                }

                let replaced_tx = existing_tx.clone();

                *existing_tx = (tx.clone(), subpool);

                Some(replaced_tx)
            }
        };

        Ok(AddTransactionOk {
            tx,
            subpool,
            replaced_tx,
        })
    }

    fn validate_tx(&self, tx: &PooledTransaction) -> Result<(), AddTransactionError> {
        let Config {
            min_protocol_basefee,
            block_gas_limit,
            ..
        } = self.config;

        if tx.max_fee_per_gas() < min_protocol_basefee {
            return Err(AddTransactionError::FeeBelowMinimumProtocolFee);
        }

        if tx.gas_limit() > block_gas_limit {
            return Err(AddTransactionError::GasLimitExceedsBlockGasLimit);
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Subpool {
    Pending = 0,
    Queued = 1,
    Basefee = 2,
}

#[derive(Debug, thiserror::Error)]
pub enum TransactionPoolError {
    #[error("{0}")]
    AddTransactionError(#[from] AddTransactionError),
}

struct AddTransactionOk {
    tx: Arc<PooledTransaction>,
    subpool: Subpool,
    replaced_tx: Option<(Arc<PooledTransaction>, Subpool)>,
}

#[derive(Debug, thiserror::Error)]
pub enum AddTransactionError {
    #[error("tx fee below minimum protocol fee")]
    FeeBelowMinimumProtocolFee,
    #[error("tx gas limit exceeds block gas limit")]
    GasLimitExceedsBlockGasLimit,
    #[error("tx nonce too low")]
    NonceTooLow,
    #[error("replacement tx underpriced")]
    ReplacementTxUnderpriced,
}

#[derive(Debug, Default)]
struct AccountInfo {
    nonce: u64,
    balance: U256,
}

impl AccountInfo {
    fn update(&mut self, nonce: u64, balance: U256) {
        self.nonce = nonce;
        self.balance = balance;
    }
}
