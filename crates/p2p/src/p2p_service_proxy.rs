use libp2p::gossipsub::{MessageId, PublishError};
use libp2p::PeerId;
use tokio::sync::{broadcast, mpsc};

use crate::gossipsub::GossipMessage;
use crate::p2p_service::TonicP2PEvent;

/// [`P2PServiceProxy`] is a struct that exists due to the singleton nature
/// of the Swarm object, which is owned by the [`crate::P2PService`]. Since it is
/// hard to share the P2PService across different components, a proxy is
/// needed to queue the requests and broadcast events from P2P network
/// to multiple subscribers. Components who wants to subscribe to P2P events,
/// or wants to create a P2P task should use [`P2PServiceProxy`] struct.
#[derive(Clone)]
pub struct P2PServiceProxy {
    /// Request-response channel to publish messages.
    /// Proxies the message request to P2PService.
    publish_message_tx: bmrng::RequestSender<GossipMessage, Result<MessageId, PublishError>>,
    /// Channel to broadcast whenever a dummy message
    /// is received from a peer.
    dummy_message_tx: broadcast::Sender<(PeerId, u64)>,
}

impl P2PServiceProxy {
    pub(crate) fn new(
        publish_message_tx: bmrng::RequestSender<GossipMessage, Result<MessageId, PublishError>>,
    ) -> Self {
        let (dummy_message_tx, _) = broadcast::channel(16);
        Self {
            publish_message_tx,
            dummy_message_tx,
        }
    }

    /// Initializes the tokio task which broadcast P2P events coming from [`crate::P2PService`]
    /// to corresponding subscriptions
    pub(crate) fn run_p2p_event_handler(
        &self,
        mut new_p2p_event_rx: mpsc::Receiver<TonicP2PEvent>,
    ) {
        let dummy_message_tx = self.dummy_message_tx.clone();
        tokio::spawn(async move {
            loop {
                let p2p_event = new_p2p_event_rx
                    .recv()
                    .await
                    .expect("New p2p event sender channel to never close");

                notify_subscribers(p2p_event, &dummy_message_tx);
            }
        });
    }

    pub fn subscribe_dummy_messages(&self) -> broadcast::Receiver<(PeerId, u64)> {
        self.dummy_message_tx.subscribe()
    }

    /// Publish a gossip message to all network peers
    pub async fn publish_message(&self, message: GossipMessage) -> Result<MessageId, PublishError> {
        self.publish_message_tx
            .send_receive(message)
            .await
            .expect("No bmrng send_receive error while publishing message")
    }
}

fn notify_subscribers(
    p2p_event: TonicP2PEvent,
    dummy_message_tx: &broadcast::Sender<(PeerId, u64)>,
) {
    match p2p_event {
        TonicP2PEvent::GossipsubMessage {
            peer_id, message, ..
        } => match message {
            GossipMessage::Dummy(value) => {
                let _ = dummy_message_tx.send((peer_id, value));
            }
        },
    };
}
