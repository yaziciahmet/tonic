use std::ops::Deref;

use borsh::{BorshDeserialize, BorshSerialize};
use tonic_primitives::{keccak256, sign_message, Address, PrimitiveSignature};

use super::codec;

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub enum IBFTMessage {
    Proposal(ProposalMessageSigned),
    Prepare(PrepareMessageSigned),
    Commit(CommitMessageSigned),
    RoundChange(RoundChangeMessageSigned),
}

impl IBFTMessage {
    pub fn ty(&self) -> MessageType {
        match self {
            Self::Proposal(m) => m.ty(),
            Self::Prepare(m) => m.ty(),
            Self::Commit(m) => m.ty(),
            Self::RoundChange(m) => m.ty(),
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
#[borsh(use_discriminant = true)]
#[repr(u8)]
pub enum MessageType {
    Proposal = 0,
    Prepare = 1,
    Commit = 2,
    RoundChange = 3,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ProposalMessage {
    pub view: View,
    pub proposed_block: ProposedBlock,
    pub proposed_block_digest: [u8; 32],
    pub round_change_certificate: Option<RoundChangeCertificate>,
}

impl ProposalMessage {
    pub fn ty(&self) -> MessageType {
        MessageType::Proposal
    }

    fn data_to_sign(&self) -> [u8; 32] {
        let bytes = codec::serialize(&(self.ty(), self.view, self.proposed_block_digest));
        *keccak256(bytes)
    }

    pub fn into_signed(self, secret: [u8; 32]) -> ProposalMessageSigned {
        let hash = self.data_to_sign();
        let signature = sign_message(secret.into(), hash.into()).expect("Signing should not fail");
        ProposalMessageSigned {
            message: self,
            signature: signature.into(),
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ProposalMessageSigned {
    pub message: ProposalMessage,
    pub signature: PrimitiveSignature,
}

impl Deref for ProposalMessageSigned {
    type Target = ProposalMessage;

    fn deref(&self) -> &Self::Target {
        &self.message
    }
}

impl ProposalMessageSigned {
    pub fn recover_signer(&self) -> anyhow::Result<Address> {
        Ok(self
            .signature
            .recover_address_from_prehash(&self.data_to_sign().into())?)
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct PrepareMessage {
    pub view: View,
    pub proposed_block_digest: [u8; 32],
}

impl PrepareMessage {
    pub fn ty(&self) -> MessageType {
        MessageType::Prepare
    }

    fn data_to_sign(&self) -> [u8; 32] {
        let bytes = codec::serialize(&(self.ty(), self.view, self.proposed_block_digest));
        *keccak256(bytes)
    }

    pub fn into_signed(self, secret: [u8; 32]) -> PrepareMessageSigned {
        let hash = self.data_to_sign();
        let signature = sign_message(secret.into(), hash.into()).expect("Signing should not fail");
        PrepareMessageSigned {
            message: self,
            signature: signature.into(),
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct PrepareMessageSigned {
    pub message: PrepareMessage,
    pub signature: PrimitiveSignature,
}

impl Deref for PrepareMessageSigned {
    type Target = PrepareMessage;

    fn deref(&self) -> &Self::Target {
        &self.message
    }
}

impl PrepareMessageSigned {
    pub fn recover_signer(&self) -> anyhow::Result<Address> {
        Ok(self
            .signature
            .recover_address_from_prehash(&self.data_to_sign().into())?)
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CommitMessage {
    pub view: View,
    pub proposed_block_digest: [u8; 32],
    pub commit_seal: PrimitiveSignature,
}

impl CommitMessage {
    pub fn ty(&self) -> MessageType {
        MessageType::Commit
    }

    fn data_to_sign(&self) -> [u8; 32] {
        let bytes = codec::serialize(&(
            self.ty(),
            self.view,
            self.proposed_block_digest,
            self.commit_seal,
        ));
        *keccak256(bytes)
    }

    pub fn into_signed(self, secret: [u8; 32]) -> CommitMessageSigned {
        let hash = self.data_to_sign();
        let signature = sign_message(secret.into(), hash.into()).expect("Signing should not fail");
        CommitMessageSigned {
            message: self,
            signature: signature.into(),
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CommitMessageSigned {
    pub message: CommitMessage,
    pub signature: PrimitiveSignature,
}

impl Deref for CommitMessageSigned {
    type Target = CommitMessage;

    fn deref(&self) -> &Self::Target {
        &self.message
    }
}

impl CommitMessageSigned {
    pub fn recover_signer(&self) -> anyhow::Result<Address> {
        Ok(self
            .signature
            .recover_address_from_prehash(&self.data_to_sign().into())?)
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct RoundChangeMessage {
    pub view: View,
    pub latest_prepared_proposed: Option<(ProposedBlock, PreparedCertificate)>,
}

impl RoundChangeMessage {
    pub fn ty(&self) -> MessageType {
        MessageType::RoundChange
    }

    fn data_to_sign(&self) -> [u8; 32] {
        let bytes = codec::serialize(&(
            self.ty(),
            self.view,
            self.latest_prepared_proposed.as_ref().map(|(_, pc)| pc),
        ));
        *keccak256(bytes)
    }

    pub fn into_signed(self, secret: [u8; 32]) -> RoundChangeMessageSigned {
        let hash = self.data_to_sign();
        let signature = sign_message(secret.into(), hash.into()).expect("Signing should not fail");
        RoundChangeMessageSigned {
            message: self,
            signature: signature.into(),
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct RoundChangeMessageSigned {
    pub message: RoundChangeMessage,
    pub signature: PrimitiveSignature,
}

impl Deref for RoundChangeMessageSigned {
    type Target = RoundChangeMessage;

    fn deref(&self) -> &Self::Target {
        &self.message
    }
}

impl RoundChangeMessageSigned {
    pub fn recover_signer(&self) -> anyhow::Result<Address> {
        Ok(self
            .signature
            .recover_address_from_prehash(&self.data_to_sign().into())?)
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct RoundChangeCertificate {
    pub round_change_messages: Vec<RoundChangeMessageSigned>,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ProposedBlock {
    pub raw_eth_block: Vec<u8>,
    pub round: u32,
}

impl ProposedBlock {
    pub fn digest(&self) -> [u8; 32] {
        let bytes = codec::serialize(self);
        *keccak256(bytes)
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct PreparedCertificate {
    pub proposal_message: ProposalMessage,
    pub prepare_messages: Vec<PrepareMessage>,
}

#[derive(Clone, Copy, Debug, BorshSerialize, BorshDeserialize)]
pub struct View {
    pub height: u64,
    pub round: u32,
}
