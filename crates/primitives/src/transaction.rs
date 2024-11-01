use primitive_types::U256;

use crate::address::Address;
use crate::chain_id::ChainId;
use crate::signature::Signature;

#[derive(Debug)]
pub enum TransactionKind {
    SendNative(Address),
    CreateToken,
}

#[derive(Debug)]
pub struct Transaction {
    pub to: TransactionKind,
    pub nonce: u64,
    pub value: U256,
    pub gas_limit: u64,
    pub max_fee_per_gas: u64,
    pub max_priority_fee_per_gas: u64,
    pub chain_id: ChainId,
}

#[derive(Debug)]
pub struct SignedTransaction {
    pub transaction: Transaction,
    pub signature: Signature,
}

#[derive(Debug)]
pub struct EcRecoveredTransaction {
    pub transaction: Transaction,
    pub signature: Signature,
    pub from: Address,
}
