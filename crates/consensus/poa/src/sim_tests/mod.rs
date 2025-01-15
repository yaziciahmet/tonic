mod mock;

use std::time::Duration;

use mock::{Mock, MockValidator};
use tokio::sync::{mpsc, oneshot};
use tonic_primitives::Signer;

use crate::backend::ValidatorManager;
use crate::engine::ConsensusEngine;
use crate::types::IBFTReceivedMessage;

#[madsim::test]
async fn ibft_run() {
    // tonic_tracing::initialize_tracing(tracing::Level::DEBUG);

    let (validators, signers, p2p_rxs) = create_validators(8);
    let mock = Mock::new(validators);

    let mut engines = Vec::with_capacity(signers.len());
    for (signer, p2p_rx) in signers.into_iter().zip(p2p_rxs) {
        let engine = ConsensusEngine::new(
            mock.clone(),
            mock.clone(),
            mock.clone(),
            mock.clone(),
            0,
            signer,
            Duration::from_secs(1),
        );
        engine.spawn_message_handler(p2p_rx);
        engines.push(engine);
    }

    // Here to keep txs alive
    let mut cancel_txs = vec![];
    for height in 1..=500 {
        let mut futs = Vec::with_capacity(engines.len());
        for engine in &engines {
            let (cancel_tx, cancel_rx) = oneshot::channel();
            cancel_txs.push(cancel_tx);
            futs.push(engine.run_height(height, cancel_rx));
        }
        let blocks = futures::future::join_all(futs).await;
        for bs in blocks.windows(2) {
            let b1 = bs[0].as_ref().unwrap();
            let b2 = bs[1].as_ref().unwrap();
            assert_eq!(b1.raw_block(), b2.raw_block());
            assert!(b1.proof().commit_seals().len() >= mock.quorum(height));
        }
    }
}

#[madsim::test]
async fn ibft_run_buggified() {
    tonic_tracing::initialize_tracing(tracing::Level::DEBUG);
    madsim::buggify::enable();

    let (validators, signers, p2p_rxs) = create_validators(4);
    let mock = Mock::new(validators);

    let mut engines = Vec::with_capacity(signers.len());
    for (signer, p2p_rx) in signers.into_iter().zip(p2p_rxs) {
        let engine = ConsensusEngine::new(
            mock.clone(),
            mock.clone(),
            mock.clone(),
            mock.clone(),
            0,
            signer,
            Duration::from_secs(1),
        );
        engine.spawn_message_handler(p2p_rx);
        engines.push(engine);
    }

    // Here to keep txs alive
    let mut cancel_txs = vec![];
    for height in 1..=500 {
        let mut futs = Vec::with_capacity(engines.len());
        for engine in &engines {
            let (cancel_tx, cancel_rx) = oneshot::channel();
            cancel_txs.push(cancel_tx);
            futs.push(engine.run_height(height, cancel_rx));
        }
        let blocks = futures::future::join_all(futs).await;
        for bs in blocks.windows(2) {
            let b1 = bs[0].as_ref().unwrap();
            let b2 = bs[1].as_ref().unwrap();
            assert_eq!(b1.raw_block(), b2.raw_block());
            assert!(b1.proof().commit_seals().len() >= mock.quorum(height));
        }
    }
}

fn create_validators(
    count: usize,
) -> (
    Vec<MockValidator>,
    Vec<Signer>,
    Vec<mpsc::Receiver<IBFTReceivedMessage>>,
) {
    let mut signers = Vec::with_capacity(count);
    let mut validators = Vec::with_capacity(count);
    let mut p2p_rxs = Vec::with_capacity(count);
    for _ in 0..count {
        let signer = Signer::random();
        let (tx, rx) = mpsc::channel(128);
        validators.push(MockValidator::new(signer.address(), tx));
        signers.push(signer);
        p2p_rxs.push(rx);
    }
    (validators, signers, p2p_rxs)
}
