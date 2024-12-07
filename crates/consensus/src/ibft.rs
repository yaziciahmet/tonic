use std::time::Duration;

use tokio::select;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tonic_primitives::PrimitiveSignature;
use tonic_signer::Signer;
use tracing::info;

use crate::backend::ValidatorManager;

use super::messages::ConsensusMessages;
use super::types::{PrepareMessage, PreparedCertificate, ProposedBlock, View};

const TIMEOUT_TABLE: [Duration; 6] = [
    Duration::from_secs(4),
    Duration::from_secs(8),
    Duration::from_secs(16),
    Duration::from_secs(32),
    Duration::from_secs(64),
    Duration::from_secs(128),
];

pub struct IBFT<V>
where
    V: ValidatorManager,
{
    messages: ConsensusMessages,
    validator_manager: V,
    signer: Signer,
}

impl<V> IBFT<V>
where
    V: ValidatorManager,
{
    pub fn new(messages: ConsensusMessages, validator_manager: V, signer: Signer) -> Self {
        Self {
            messages,
            validator_manager,
            signer,
        }
    }

    pub async fn run(&self, height: u64, mut cancel: oneshot::Receiver<()>) {
        let mut view = View { height, round: 0 };

        info!("Running consensus height {}", view.height);
        loop {
            info!("Running consensus round {}", view.round);

            let timeout = tokio::time::sleep(get_round_timeout(view.round));
            let (future_proposal_rx, future_proposal_task) = self.watch_future_proposal(view);
            let (rcc_rx, rcc_task) = self.watch_rcc(view);
            let (round_finished, round_task) = self.start_ibft_round(view);

            let abort = move || {
                round_task.abort();
                future_proposal_task.abort();
                rcc_task.abort();
            };

            select! {
                biased;
                _ = &mut cancel => {
                    info!("Received cancel signal, stopping consensus...");
                    abort();
                    return;
                }
                _ = timeout => {
                    info!("Round timeout");
                }
                _ = future_proposal_rx => {
                }
                _ = rcc_rx => {
                }
                _ = round_finished => {
                }
            }

            abort();
            view.round += 1;
        }
    }

    fn start_ibft_round(&self, view: View) -> (oneshot::Receiver<()>, JoinHandle<()>) {
        let messages = self.messages.clone();
        let validator_manager = self.validator_manager.clone();
        let signer = self.signer.clone();

        let state = RunState::new(view);

        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = tx.send(());
        });

        (rx, task)
    }

    fn watch_rcc(&self, view: View) -> (oneshot::Receiver<()>, JoinHandle<()>) {
        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            // TODO: actually watch for rcc
            tokio::time::sleep(Duration::from_secs(9999)).await;
            let _ = tx.send(());
        });

        (rx, task)
    }

    fn watch_future_proposal(&self, view: View) -> (oneshot::Receiver<()>, JoinHandle<()>) {
        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            // TODO: actually watch for future proposal
            tokio::time::sleep(Duration::from_secs(9999)).await;
            let _ = tx.send(());
        });

        (rx, task)
    }
}

fn get_round_timeout(mut round: u32) -> Duration {
    if round > 5 {
        round = 5;
    }

    TIMEOUT_TABLE[round as usize]
}

#[derive(Debug)]
struct RunState {
    view: View,
    proposed_block: Option<ProposedBlock>,
    proposed_block_digest: Option<[u8; 32]>,
    valid_prepare_messages: Vec<PrepareMessage>,
    latest_prepared_certificate: Option<PreparedCertificate>,
    valid_commit_seals: Vec<PrimitiveSignature>,
}

impl RunState {
    fn new(view: View) -> Self {
        Self {
            view,
            proposed_block: None,
            proposed_block_digest: None,
            valid_prepare_messages: vec![],
            latest_prepared_certificate: None,
            valid_commit_seals: vec![],
        }
    }

    fn view(&self) -> View {
        self.view
    }
}
