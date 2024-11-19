use std::cmp;

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
    pub from: Address,
    pub to: TransactionKind,
    pub nonce: u64,
    pub value: U256,
    pub gas_limit: u64,
    pub max_fee_per_gas: u64,
    pub max_priority_fee_per_gas: u64,
    pub chain_id: ChainId,
}

impl Transaction {
    /// Returns the final tip per gas as `min(max_fee_per_gas - basefee, max_priority_fee_per_gas)`
    pub fn effective_tip_per_gas(&self, basefee: u64) -> u64 {
        cmp::min(
            self.max_fee_per_gas - basefee,
            self.max_priority_fee_per_gas,
        )
    }

    /// Returns total cost of the transaction as `value + gas_limit * (tip + basefee)`
    pub fn cost(&self, basefee: u64) -> U256 {
        self.value + self.gas_limit * (self.effective_tip_per_gas(basefee) + basefee)
    }
}

#[derive(Debug)]
pub struct SignedTransaction {
    pub tx: Transaction,
    pub signature: Signature,
}
