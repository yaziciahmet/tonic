use std::io;
use std::ops::Deref;

pub use alloy_consensus::{Transaction as TransactionT, TxEip1559, TxEip2930, TxEip4844, TxLegacy};
pub use alloy_eips::eip2930::AccessList;
pub use alloy_eips::eip7702::SignedAuthorization;
pub use alloy_primitives::{
    address, hex, keccak256, Address, BlockHash, BlockNumber, Bytes, ChainId, FixedBytes,
    PrimitiveSignature as AlloyPrimitiveSignature, TxHash, TxKind, B256, U256,
};
use borsh::{BorshDeserialize, BorshSerialize};
pub use reth_primitives::{
    sign_message, Account, Block, BlockBody, BlockWithSenders, Transaction, TransactionSigned,
    TransactionSignedEcRecovered, TransactionSignedNoHash,
};

#[derive(Debug, Clone, Copy)]
pub struct PrimitiveSignature(AlloyPrimitiveSignature);

impl Deref for PrimitiveSignature {
    type Target = AlloyPrimitiveSignature;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<AlloyPrimitiveSignature> for PrimitiveSignature {
    fn from(sig: AlloyPrimitiveSignature) -> Self {
        Self(sig)
    }
}

impl From<PrimitiveSignature> for AlloyPrimitiveSignature {
    fn from(sig: PrimitiveSignature) -> Self {
        sig.0
    }
}

impl BorshSerialize for PrimitiveSignature {
    fn serialize<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.0.as_bytes())
    }
}

impl BorshDeserialize for PrimitiveSignature {
    fn deserialize_reader<R: io::Read>(reader: &mut R) -> io::Result<Self> {
        let mut buf = [0; 65];
        reader.read_exact(&mut buf)?;

        let inner = AlloyPrimitiveSignature::try_from(&buf[..])
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        Ok(PrimitiveSignature(inner))
    }
}
