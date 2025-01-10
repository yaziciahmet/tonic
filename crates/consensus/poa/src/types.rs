use borsh::{BorshDeserialize, BorshSerialize};
use tonic_primitives::crypto::sha256;
use tonic_primitives::{Address, Signature, Signer};

use super::codec;

#[derive(Debug, BorshSerialize)]
pub enum IBFTBroadcastMessage<'a> {
    Proposal(&'a ProposalMessageSigned),
    Prepare(&'a PrepareMessageSigned),
    Commit(&'a CommitMessageSigned),
    RoundChange(&'a RoundChangeMessageSigned),
}

#[derive(Debug, BorshDeserialize)]
pub enum IBFTReceivedMessage {
    Proposal(ProposalMessageSigned),
    Prepare(PrepareMessageSigned),
    Commit(CommitMessageSigned),
    RoundChange(RoundChangeMessageSigned),
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
        raw_block: Vec<u8>,
        round_change_certificate: Option<RoundChangeCertificate>,
    ) -> Self {
        let proposed_block = ProposedBlock {
            raw_block,
            round: view.round,
        };
        Self {
            view,
            proposed_block_digest: proposed_block.digest(),
            proposed_block,
            round_change_certificate,
        }
    }

    pub fn digest(&self) -> [u8; 32] {
        digest_proposal(&self.view, &self.proposed_block_digest)
    }

    pub fn into_signed(self, signer: &Signer) -> ProposalMessageSigned {
        let prehash = self.digest();
        let signature = signer.sign_prehash(prehash);
        ProposalMessageSigned {
            message: self,
            signature,
        }
    }

    pub fn verify_block_digest(&self) -> bool {
        self.proposed_block.digest() == self.proposed_block_digest
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ProposalMessageSigned {
    message: ProposalMessage,
    signature: Signature,
}

impl ProposalMessageSigned {
    pub fn view(&self) -> View {
        self.message.view
    }

    pub fn into_proposed_block(self) -> ProposedBlock {
        self.message.proposed_block
    }

    pub fn proposed_block(&self) -> &ProposedBlock {
        &self.message.proposed_block
    }

    pub fn proposed_block_digest(&self) -> [u8; 32] {
        self.message.proposed_block_digest
    }

    pub fn round_change_certificate(&self) -> Option<&RoundChangeCertificate> {
        self.message.round_change_certificate.as_ref()
    }

    pub fn recover_signer(&self) -> anyhow::Result<Address> {
        self.signature.recover_from_prehash(self.message.digest())
    }

    pub fn verify_block_digest(&self) -> bool {
        self.message.verify_block_digest()
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ProposalMetadata {
    view: View,
    proposed_block_digest: [u8; 32],
    signature: Signature,
}

impl ProposalMetadata {
    pub fn view(&self) -> View {
        self.view
    }

    pub fn proposed_block_digest(&self) -> [u8; 32] {
        self.proposed_block_digest
    }

    pub fn recover_signer(&self) -> anyhow::Result<Address> {
        self.signature
            .recover_from_prehash(digest_proposal(&self.view, &self.proposed_block_digest))
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct PrepareMessage {
    view: View,
    proposed_block_digest: [u8; 32],
}

impl PrepareMessage {
    pub fn new(view: View, proposed_block_digest: [u8; 32]) -> Self {
        Self {
            view,
            proposed_block_digest,
        }
    }

    fn digest(&self) -> [u8; 32] {
        digest_prepare(&self.view, &self.proposed_block_digest)
    }

    pub fn into_signed(self, signer: &Signer) -> PrepareMessageSigned {
        let prehash = self.digest();
        let signature = signer.sign_prehash(prehash);
        PrepareMessageSigned {
            message: self,
            signature,
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct PrepareMessageSigned {
    message: PrepareMessage,
    signature: Signature,
}

impl PrepareMessageSigned {
    pub fn view(&self) -> View {
        self.message.view
    }

    pub fn proposed_block_digest(&self) -> [u8; 32] {
        self.message.proposed_block_digest
    }

    pub fn recover_signer(&self) -> anyhow::Result<Address> {
        self.signature.recover_from_prehash(self.message.digest())
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CommitMessage {
    view: View,
    proposed_block_digest: [u8; 32],
    commit_seal: Signature,
}

impl CommitMessage {
    pub fn new(view: View, proposed_block_digest: [u8; 32], signer: &Signer) -> Self {
        let commit_seal = signer.sign_prehash(proposed_block_digest);
        Self {
            view,
            proposed_block_digest,
            commit_seal,
        }
    }

    pub fn recover_commit_seal_signer(&self) -> anyhow::Result<Address> {
        self.commit_seal
            .recover_from_prehash(self.proposed_block_digest)
    }

    fn digest(&self) -> [u8; 32] {
        digest_commit(&self.view, &self.proposed_block_digest, &self.commit_seal)
    }

    pub fn into_signed(self, signer: &Signer) -> CommitMessageSigned {
        let prehash = self.digest();
        let signature = signer.sign_prehash(prehash);
        CommitMessageSigned {
            message: self,
            signature,
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CommitMessageSigned {
    message: CommitMessage,
    signature: Signature,
}

impl CommitMessageSigned {
    pub fn view(&self) -> View {
        self.message.view
    }

    pub fn proposed_block_digest(&self) -> [u8; 32] {
        self.message.proposed_block_digest
    }

    pub fn commit_seal(&self) -> Signature {
        self.message.commit_seal
    }

    pub fn recover_commit_seal_signer(&self) -> anyhow::Result<Address> {
        self.message.recover_commit_seal_signer()
    }

    pub fn recover_signer(&self) -> anyhow::Result<Address> {
        self.signature.recover_from_prehash(self.message.digest())
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct PreparedProposed {
    proposed_block: ProposedBlock,
    prepared_certificate: PreparedCertificate,
}

impl PreparedProposed {
    pub fn new(proposal: ProposalMessageSigned, prepares: Vec<PrepareMessageSigned>) -> Self {
        let proposal_meta = ProposalMetadata {
            view: proposal.message.view,
            proposed_block_digest: proposal.message.proposed_block_digest,
            signature: proposal.signature,
        };
        let proposed_block = proposal.message.proposed_block;

        Self {
            proposed_block,
            prepared_certificate: PreparedCertificate::new(proposal_meta, prepares),
        }
    }

    pub fn proposed_block(&self) -> &ProposedBlock {
        &self.proposed_block
    }

    pub fn prepared_certificate(&self) -> &PreparedCertificate {
        &self.prepared_certificate
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct RoundChangeMessage {
    view: View,
    latest_prepared_proposed: Option<PreparedProposed>,
}

impl RoundChangeMessage {
    pub fn new(view: View, latest_prepared_proposed: Option<PreparedProposed>) -> Self {
        Self {
            view,
            latest_prepared_proposed,
        }
    }

    fn digest(&self) -> [u8; 32] {
        digest_round_change(
            &self.view,
            self.latest_prepared_proposed
                .as_ref()
                .map(|p| &p.prepared_certificate),
        )
    }

    pub fn into_signed(self, signer: &Signer) -> RoundChangeMessageSigned {
        let prehash = self.digest();
        let signature = signer.sign_prehash(prehash);
        RoundChangeMessageSigned {
            message: self,
            signature,
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct RoundChangeMessageSigned {
    message: RoundChangeMessage,
    signature: Signature,
}

impl RoundChangeMessageSigned {
    pub fn view(&self) -> View {
        self.message.view
    }

    /// Updates the round of the round change message, and resigns the message.
    pub fn update_and_resign(&mut self, round: u32, signer: &Signer) {
        self.message.view.round = round;
        let prehash = self.message.digest();
        self.signature = signer.sign_prehash(prehash);
    }

    pub fn latest_prepared_proposed(&self) -> &Option<PreparedProposed> {
        &self.message.latest_prepared_proposed
    }

    pub fn recover_signer(&self) -> anyhow::Result<Address> {
        self.signature.recover_from_prehash(self.message.digest())
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct RoundChangeMetadata {
    view: View,
    latest_prepared_certificate: Option<PreparedCertificate>,
    signature: Signature,
}

impl RoundChangeMetadata {
    pub fn from_message(message: RoundChangeMessageSigned) -> Self {
        Self {
            view: message.view(),
            latest_prepared_certificate: message
                .message
                .latest_prepared_proposed
                .map(|p| p.prepared_certificate),
            signature: message.signature,
        }
    }

    pub fn view(&self) -> View {
        self.view
    }

    pub fn latest_prepared_certificate(&self) -> &Option<PreparedCertificate> {
        &self.latest_prepared_certificate
    }

    fn digest(&self) -> [u8; 32] {
        digest_round_change(&self.view, self.latest_prepared_certificate.as_ref())
    }

    pub fn recover_signer(&self) -> anyhow::Result<Address> {
        self.signature.recover_from_prehash(self.digest())
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct RoundChangeCertificate {
    pub round_change_messages: Vec<RoundChangeMetadata>,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ProposedBlock {
    raw_block: Vec<u8>,
    round: u32,
}

impl ProposedBlock {
    pub fn new(raw_block: Vec<u8>, round: u32) -> Self {
        Self { raw_block, round }
    }

    pub fn raw_block(&self) -> &[u8] {
        &self.raw_block
    }

    pub fn round(&self) -> u32 {
        self.round
    }

    pub fn digest(&self) -> [u8; 32] {
        digest_block(&self.raw_block, self.round)
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct PreparedCertificate {
    proposal_meta: ProposalMetadata,
    prepare_messages: Vec<PrepareMessageSigned>,
}

impl PreparedCertificate {
    pub fn new(proposal_meta: ProposalMetadata, prepares: Vec<PrepareMessageSigned>) -> Self {
        Self {
            proposal_meta,
            prepare_messages: prepares,
        }
    }

    pub fn proposal(&self) -> &ProposalMetadata {
        &self.proposal_meta
    }

    pub fn prepare_messages(&self) -> &[PrepareMessageSigned] {
        &self.prepare_messages
    }
}

pub type CommitSeals = Vec<Signature>;

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct FinalizationProof {
    round: u32,
    commit_seals: CommitSeals,
}

impl FinalizationProof {
    pub fn new(round: u32, commit_seals: CommitSeals) -> Self {
        Self {
            round,
            commit_seals,
        }
    }

    pub fn round(&self) -> u32 {
        self.round
    }

    pub fn commit_seals(&self) -> &[Signature] {
        &self.commit_seals
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct FinalizedBlock {
    raw_block: Vec<u8>,
    proof: FinalizationProof,
}

impl FinalizedBlock {
    pub fn new(proposed_block: ProposedBlock, commit_seals: CommitSeals) -> Self {
        Self {
            proof: FinalizationProof::new(proposed_block.round, commit_seals),
            raw_block: proposed_block.raw_block,
        }
    }

    pub fn raw_block(&self) -> &[u8] {
        &self.raw_block
    }

    pub fn proof(&self) -> &FinalizationProof {
        &self.proof
    }
}

#[derive(Clone, Copy, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct View {
    pub height: u64,
    pub round: u32,
}

impl View {
    pub fn new(height: u64, round: u32) -> Self {
        Self { height, round }
    }
}

pub fn digest_proposal(view: &View, proposed_block_digest: &[u8; 32]) -> [u8; 32] {
    let bytes = codec::serialize(&(MessageType::Proposal, view, proposed_block_digest));
    sha256(&bytes)
}

pub fn digest_prepare(view: &View, proposed_block_digest: &[u8; 32]) -> [u8; 32] {
    let bytes = codec::serialize(&(MessageType::Prepare, view, proposed_block_digest));
    sha256(&bytes)
}

pub fn digest_commit(
    view: &View,
    proposed_block_digest: &[u8; 32],
    commit_seal: &Signature,
) -> [u8; 32] {
    let bytes = codec::serialize(&(
        MessageType::Commit,
        view,
        proposed_block_digest,
        commit_seal,
    ));
    sha256(&bytes)
}

pub fn digest_round_change(
    view: &View,
    latest_prepared_certificate: Option<&PreparedCertificate>,
) -> [u8; 32] {
    let bytes = codec::serialize(&(MessageType::RoundChange, view, latest_prepared_certificate));
    sha256(&bytes)
}

pub fn digest_block(raw_block: &[u8], round: u32) -> [u8; 32] {
    let data = codec::serialize(&(raw_block, round));
    sha256(&data)
}
