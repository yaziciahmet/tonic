use std::time::Duration;

use tokio::select;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tonic_primitives::{Address, PrimitiveSignature};
use tracing::info;

use crate::validator_manager::ValidatorManager;

use super::messages::ConsensusMessages;
use super::types::{PrepareMessage, PreparedCertificate, ProposedBlock, View};

pub struct IBFT<V>
where
    V: ValidatorManager,
{
    base_round_timeout: Duration,
    messages: ConsensusMessages,
    validator_manager: V,
    address: Address,
}

impl<V> IBFT<V>
where
    V: ValidatorManager,
{
    pub fn new(messages: ConsensusMessages, validator_manager: V, address: Address) -> Self {
        Self {
            base_round_timeout: Duration::from_secs(8),
            messages,
            validator_manager,
            address,
        }
    }

    pub async fn run(&self, height: u64, mut cancel: oneshot::Receiver<()>) {
        let mut state = RunState::new(View { height, round: 0 });

        loop {
            let view = state.view();

            info!(
                height = view.height,
                round = view.round,
                "Running consensus"
            );

            let timeout = tokio::time::sleep(self.get_round_timeout(view.round));
            let (future_proposal_rx, future_proposal_task) = self.watch_future_proposal();
            let (rcc_rx, rcc_task) = self.watch_rcc();
            let (round_finished, round_task) = self.start_ibft_round(&mut state);

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
                    abort();
                }
                _ = future_proposal_rx => {
                    abort();
                }
                _ = rcc_rx => {
                    abort();
                }
                _ = round_finished => {
                    abort();
                }
            }
        }
    }

    fn start_ibft_round(&self, _state: &mut RunState) -> (oneshot::Receiver<()>, JoinHandle<()>) {
        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            // TODO: actually run state transition
            tokio::time::sleep(Duration::from_secs(9999)).await;
            let _ = tx.send(());
        });

        (rx, task)
    }

    fn watch_rcc(&self) -> (oneshot::Receiver<()>, JoinHandle<()>) {
        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            // TODO: actually watch for rcc
            tokio::time::sleep(Duration::from_secs(9999)).await;
            let _ = tx.send(());
        });

        (rx, task)
    }

    fn watch_future_proposal(&self) -> (oneshot::Receiver<()>, JoinHandle<()>) {
        let (tx, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            // TODO: actually watch for future proposal
            tokio::time::sleep(Duration::from_secs(9999)).await;
            let _ = tx.send(());
        });

        (rx, task)
    }

    pub fn get_round_timeout(&self, _round: u32) -> Duration {
        self.base_round_timeout
    }
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
