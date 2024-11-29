use std::time::Duration;

use tokio::select;
use tokio::sync::oneshot;
use tonic_primitives::PrimitiveSignature;
use tracing::info;

use super::messages::ConsensusMessages;
use super::types::{PrepareMessage, PreparedCertificate, ProposedBlock, View};

pub struct IBFT {
    base_round_timeout: Duration,
}

impl IBFT {
    pub fn new() -> Self {
        Self {
            base_round_timeout: Duration::from_secs(8),
        }
    }

    pub async fn run(
        &self,
        height: u64,
        messages: &ConsensusMessages,
        mut cancel: oneshot::Receiver<()>,
    ) {
        let mut state = RunState::new(View { height, round: 0 });

        loop {
            let view = state.view();

            info!(
                height = view.height,
                round = view.round,
                "Running consensus"
            );

            let timeout = tokio::time::sleep(self.get_round_timeout(view.round));
            let rcc = self.wait_for_rcc(messages);
            let future_proposal = self.wait_for_future_proposal();
            let finalized_block = self.run_state_transition(&mut state);

            select! {
                _ = finalized_block => {}
                _ = rcc => {}
                _ = future_proposal => {}
                _ = timeout => {
                    info!("Round timeout");
                    state.move_round(view.round + 1);
                    // TODO: generate round change message
                }
                _ = &mut cancel => {
                    info!("Received cancel signal, stopping consensus...");
                    return;
                }
            }
        }
    }

    async fn run_state_transition(&self, _state: &mut RunState) {
        todo!()
    }

    async fn wait_for_rcc(&self, messages: &ConsensusMessages) {
        let _rx = messages.subscribe_round_change();
        todo!()
    }

    async fn wait_for_future_proposal(&self) {
        todo!()
    }

    pub fn get_round_timeout(&self, _round: u32) -> Duration {
        self.base_round_timeout
    }
}

#[derive(Debug)]
pub struct RunState {
    view: View,
    proposed_block: Option<ProposedBlock>,
    proposed_block_digest: Option<[u8; 32]>,
    valid_prepare_messages: Vec<PrepareMessage>,
    latest_prepared_certificate: Option<PreparedCertificate>,
    valid_commit_seals: Vec<PrimitiveSignature>,
}

impl RunState {
    pub fn new(view: View) -> Self {
        Self {
            view,
            proposed_block: None,
            proposed_block_digest: None,
            valid_prepare_messages: vec![],
            latest_prepared_certificate: None,
            valid_commit_seals: vec![],
        }
    }

    pub fn view(&self) -> View {
        self.view
    }

    pub fn move_round(&mut self, round: u32) -> Option<(PreparedCertificate, ProposedBlock)> {
        self.view.round = round;

        let proposed_block = self.proposed_block.take();
        self.proposed_block_digest = None;
        self.valid_prepare_messages.clear();
        let latest_pc = self.latest_prepared_certificate.take();
        self.valid_commit_seals.clear();

        match (latest_pc, proposed_block) {
            (Some(pc), Some(block)) => Some((pc, block)),
            (Some(_), None) => panic!("Has prepared certificate but doesn't have proposed block"),
            (None, _) => None,
        }
    }
}
