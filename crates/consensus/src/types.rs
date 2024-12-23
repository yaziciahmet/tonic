use std::io;
use std::sync::Arc;

use borsh::{BorshDeserialize, BorshSerialize};
use tonic_primitives::{keccak256, Address, PrimitiveSignature, B256};
use tonic_signer::Signer;

use super::codec;

#[derive(Debug)]
pub enum IBFTMessage {
    Proposal(Arc<ProposalMessageSigned>),
    Prepare(Arc<PrepareMessageSigned>),
    Commit(Arc<CommitMessageSigned>),
    RoundChange(Arc<RoundChangeMessageSigned>),
}

impl BorshSerialize for IBFTMessage {
    fn serialize<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Proposal(proposal) => {
                // Write the enum variant
                proposal.message.ty().serialize(writer)?;
                // Write inner data
                proposal.serialize(writer)
            }
            Self::Prepare(prepare) => {
                // Write the enum variant
                prepare.message.ty().serialize(writer)?;
                // Write inner data
                prepare.serialize(writer)
            }
            Self::Commit(commit) => {
                // Write the enum variant
                commit.message.ty().serialize(writer)?;
                // Write inner data
                commit.serialize(writer)
            }
            Self::RoundChange(round_change) => {
                // Write the enum variant
                round_change.message.ty().serialize(writer)?;
                // Write inner data
                round_change.serialize(writer)
            }
        }
    }
}

impl BorshDeserialize for IBFTMessage {
    fn deserialize_reader<R: io::Read>(reader: &mut R) -> io::Result<Self> {
        let ty = borsh::from_reader(reader)?;
        match ty {
            MessageType::Proposal => {
                let message = borsh::from_reader(reader)?;
                Ok(Self::Proposal(Arc::new(message)))
            }
            MessageType::Prepare => {
                let message = borsh::from_reader(reader)?;
                Ok(Self::Prepare(Arc::new(message)))
            }
            MessageType::Commit => {
                let message = borsh::from_reader(reader)?;
                Ok(Self::Commit(Arc::new(message)))
            }
            MessageType::RoundChange => {
                let message = borsh::from_reader(reader)?;
                Ok(Self::RoundChange(Arc::new(message)))
            }
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
    view: View,
    proposed_block: ProposedBlock,
    proposed_block_digest: [u8; 32],
    round_change_certificate: Option<RoundChangeCertificate>,
}

impl ProposalMessage {
    pub fn new(
        view: View,
        raw_eth_block: Vec<u8>,
        round_change_certificate: Option<RoundChangeCertificate>,
    ) -> Self {
        let proposed_block = ProposedBlock {
            raw_eth_block,
            round: view.round,
        };
        Self {
            view,
            proposed_block_digest: proposed_block.digest(),
            proposed_block,
            round_change_certificate,
        }
    }

    pub fn ty(&self) -> MessageType {
        MessageType::Proposal
    }

    pub fn view(&self) -> View {
        self.view
    }

    pub fn proposed_block(&self) -> &ProposedBlock {
        &self.proposed_block
    }

    fn data_to_sign(&self) -> B256 {
        let bytes = codec::serialize(&(self.ty(), self.view, self.proposed_block_digest));
        keccak256(bytes)
    }

    pub fn into_signed(self, signer: &Signer) -> ProposalMessageSigned {
        let hash = self.data_to_sign();
        let signature = signer.sign_prehashed(hash);
        ProposalMessageSigned {
            message: self,
            signature,
        }
    }

    pub fn verify_digest(&self) -> bool {
        self.proposed_block.digest() == self.proposed_block_digest
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ProposalMessageSigned {
    message: ProposalMessage,
    signature: PrimitiveSignature,
}

impl ProposalMessageSigned {
    pub fn view(&self) -> View {
        self.message.view
    }

    pub fn proposed_block(&self) -> &ProposedBlock {
        &self.message.proposed_block
    }

    pub fn recover_signer(&self) -> anyhow::Result<Address> {
        Ok(self
            .signature
            .recover_address_from_prehash(&self.message.data_to_sign())?)
    }

    pub fn verify_digest(&self) -> bool {
        self.message.verify_digest()
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct PrepareMessage {
    view: View,
    proposed_block_digest: [u8; 32],
}

impl PrepareMessage {
    pub fn ty(&self) -> MessageType {
        MessageType::Prepare
    }

    pub fn view(&self) -> View {
        self.view
    }

    fn data_to_sign(&self) -> B256 {
        let bytes = codec::serialize(&(self.ty(), self.view, self.proposed_block_digest));
        keccak256(bytes)
    }

    pub fn into_signed(self, signer: &Signer) -> PrepareMessageSigned {
        let hash = self.data_to_sign();
        let signature = signer.sign_prehashed(hash);
        PrepareMessageSigned {
            message: self,
            signature,
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct PrepareMessageSigned {
    message: PrepareMessage,
    signature: PrimitiveSignature,
}

impl PrepareMessageSigned {
    pub fn view(&self) -> View {
        self.message.view
    }

    pub fn recover_signer(&self) -> anyhow::Result<Address> {
        Ok(self
            .signature
            .recover_address_from_prehash(&self.message.data_to_sign())?)
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CommitMessage {
    view: View,
    proposed_block_digest: [u8; 32],
    commit_seal: PrimitiveSignature,
}

impl CommitMessage {
    pub fn ty(&self) -> MessageType {
        MessageType::Commit
    }

    pub fn view(&self) -> View {
        self.view
    }

    fn data_to_sign(&self) -> B256 {
        let bytes = codec::serialize(&(
            self.ty(),
            self.view,
            self.proposed_block_digest,
            self.commit_seal,
        ));
        keccak256(bytes)
    }

    pub fn into_signed(self, signer: &Signer) -> CommitMessageSigned {
        let hash = self.data_to_sign();
        let signature = signer.sign_prehashed(hash);
        CommitMessageSigned {
            message: self,
            signature,
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CommitMessageSigned {
    message: CommitMessage,
    signature: PrimitiveSignature,
}

impl CommitMessageSigned {
    pub fn view(&self) -> View {
        self.message.view
    }

    pub fn recover_signer(&self) -> anyhow::Result<Address> {
        Ok(self
            .signature
            .recover_address_from_prehash(&self.message.data_to_sign())?)
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct RoundChangeMessage {
    view: View,
    latest_prepared_proposed: Option<(ProposedBlock, PreparedCertificate)>,
}

impl RoundChangeMessage {
    pub fn ty(&self) -> MessageType {
        MessageType::RoundChange
    }

    pub fn view(&self) -> View {
        self.view
    }

    fn data_to_sign(&self) -> B256 {
        let bytes = codec::serialize(&(
            self.ty(),
            self.view,
            self.latest_prepared_proposed.as_ref().map(|(_, pc)| pc),
        ));
        keccak256(bytes)
    }

    pub fn into_signed(self, signer: &Signer) -> RoundChangeMessageSigned {
        let hash = self.data_to_sign();
        let signature = signer.sign_prehashed(hash);
        RoundChangeMessageSigned {
            message: self,
            signature,
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct RoundChangeMessageSigned {
    message: RoundChangeMessage,
    signature: PrimitiveSignature,
}

impl RoundChangeMessageSigned {
    pub fn view(&self) -> View {
        self.message.view
    }

    pub fn recover_signer(&self) -> anyhow::Result<Address> {
        Ok(self
            .signature
            .recover_address_from_prehash(&self.message.data_to_sign())?)
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct RoundChangeCertificate {
    pub round_change_messages: Vec<RoundChangeMessageSigned>,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ProposedBlock {
    raw_eth_block: Vec<u8>,
    round: u32,
}

impl ProposedBlock {
    pub fn raw_eth_block(&self) -> &[u8] {
        &self.raw_eth_block
    }

    pub fn round(&self) -> u32 {
        self.round
    }
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

#[derive(Clone, Copy, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct View {
    pub height: u64,
    pub round: u32,
}
