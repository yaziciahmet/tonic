use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;

use primitive_types::U256;
use tonic_primitives::address::Address;

use crate::config::Config;
use crate::parked::ParkedPool;
use crate::pending::PendingPool;
use crate::transaction::{PoolTransaction, TransactionId};

pub struct TransactionPool {
    pending_pool: PendingPool,
    // 1. account balance is not enough
    // 2. account nonce has gaps
    queued_pool: ParkedPool,
    // Dynamic basefee conditions are not met, but might be met in the future
    basefee_pool: ParkedPool,
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
    config: Config,
    txs: BTreeMap<TransactionId, PoolTransactionWithSubpool>,
    account_info: BTreeMap<Address, AccountInfo>,
}

impl AllTransactions {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            txs: BTreeMap::default(),
            account_info: BTreeMap::default(),
        }
    }

    fn add_transaction(
        &mut self,
        tx: PoolTransaction,
        basefee: u64,
    ) -> Result<AddTransactionOk, AddTransactionErr> {
        // TODO: check if this tx hash exists already
        let Config {
            min_protocol_basefee,
            max_tx_count,
            max_tx_per_account,
            max_pool_size,
        } = self.config;
        assert!(
            basefee >= min_protocol_basefee,
            "Base fee can not be less than minimum protocol fee"
        );

        let inner_tx = &tx.tx;

        if inner_tx.max_fee_per_gas < min_protocol_basefee {
            return Err(AddTransactionErr::BaseFeeTooLow);
        }

        // TODO: check if this is a replacement tx, if so, check if it increased tip at least 10%

        let account = self.account_info.get(&inner_tx.from).unwrap();
        let move_to = match inner_tx.nonce.cmp(&account.nonce) {
            Ordering::Equal => {
                if account.balance < inner_tx.cost(basefee) {
                    Subpool::Pending
                } else {
                    Subpool::Queued
                }
            }
            Ordering::Greater => Subpool::Queued,
            Ordering::Less => {
                return Err(AddTransactionErr::NonceTooLow);
            }
        };

        let tx = Arc::new(tx);

        todo!()
    }
}

#[derive(Clone, Debug)]
enum Subpool {
    Pending = 0,
    Queued = 1,
    Basefee = 2,
}

#[derive(Clone, Debug)]
struct PoolTransactionWithSubpool {
    tx: Arc<PoolTransaction>,
    subpool: Subpool,
}

struct AddTransactionOk {
    tx: Arc<PoolTransaction>,
    replaced_tx: Option<(Arc<PoolTransaction>, Subpool)>,
    move_to: Subpool,
}

enum AddTransactionErr {
    BaseFeeTooLow,
    NonceTooLow,
}

#[derive(Debug)]
struct AccountInfo {
    address: Address,
    nonce: u64,
    balance: U256,
}
