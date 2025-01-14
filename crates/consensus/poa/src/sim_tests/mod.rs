mod mock;

use std::time::Duration;

use mock::Mock;
use tokio::sync::oneshot;
use tonic_primitives::Signer;

use crate::engine::ConsensusEngine;

#[madsim::test]
async fn ibft_run() {
    tonic_tracing::initialize_tracing(tracing::Level::DEBUG);

    let mock = Mock {};
    let signer = Signer::random();
    let engine = ConsensusEngine::new(
        mock.clone(),
        mock.clone(),
        mock.clone(),
        mock,
        1,
        signer,
        Duration::from_secs(1),
    );

    let (_tx, rx) = oneshot::channel();
    let _finalized_block = engine.run_height(2, rx).await;
    let (_tx, rx) = oneshot::channel();
    let finalized_block = engine.run_height(3, rx).await;
    assert!(finalized_block.is_some());
}
