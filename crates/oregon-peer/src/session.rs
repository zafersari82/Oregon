use std::net::SocketAddr;

use oregon_network::TransportConnection;
use oregon_protocol::{
    FRAME_HEADER_BYTES, FeatureSet, Hash256, InventoryItem, InventoryKind, Message, encode_message,
};
use tokio::time::Instant;

use crate::budget::PeerQueueBudget;
use crate::service::{LivenessAction, LivenessState, Registration};
use crate::{
    Direction, PeerError, PeerFeedback, PeerScore, PerformanceSnapshot, QueueClass, RequestError,
    RequestKey, RequestRegistry, ResponseDisposition, ScoreDecision,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstablishedPeer {
    pub peer_id: PeerId,
    pub remote_addr: SocketAddr,
    pub direction: Direction,
    pub negotiated_version: u16,
    pub features: FeatureSet,
    pub remote_best_height: u64,
    pub remote_best_block_id: Hash256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    Requested,
    Network,
    Handshake,
    Replaced,
    QueueTimeout,
    Misbehavior,
    LivenessTimeout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerEvent {
    Established(EstablishedPeer),
    Message {
        peer_id: PeerId,
        message: Message,
    },
    MatchedResponse {
        peer_id: PeerId,
        key: RequestKey,
        message: Message,
    },
    RequestTimedOut {
        peer_id: PeerId,
        key: RequestKey,
    },
    Unsolicited {
        peer_id: PeerId,
        message: Message,
    },
    Disconnected {
        peer_id: PeerId,
        reason: DisconnectReason,
    },
}

pub struct PeerSession<C: TransportConnection> {
    peer: EstablishedPeer,
    connection: C,
    budget: PeerQueueBudget,
    requests: RequestRegistry,
    score: PeerScore,
    liveness: LivenessState,
    _registration: Registration,
}

impl<C: TransportConnection> PeerSession<C> {
    pub(crate) fn new(
        peer: EstablishedPeer,
        connection: C,
        budget: PeerQueueBudget,
        registration: Registration,
    ) -> Self {
        Self {
            peer,
            connection,
            budget,
            requests: RequestRegistry::default(),
            score: PeerScore::default(),
            liveness: LivenessState::new(Instant::now()),
            _registration: registration,
        }
    }

    pub fn peer(&self) -> &EstablishedPeer {
        &self.peer
    }

    pub fn remote_addr(&self) -> SocketAddr {
        self.connection.remote_addr()
    }

    pub fn expect(&mut self, key: RequestKey) -> Result<(), RequestError> {
        self.requests.expect(key)
    }

    pub fn performance(&self) -> PerformanceSnapshot {
        self.requests.performance()
    }

    pub fn score(&self) -> PeerScore {
        self.score
    }

    pub fn sync_eligible(&self) -> bool {
        self.score.sync_eligible()
    }

    pub fn disconnect_required(&self) -> bool {
        self.score.disconnect_required()
    }

    pub fn apply_feedback(&mut self, feedback: PeerFeedback) -> ScoreDecision {
        self.score.apply(feedback.misbehavior())
    }

    pub fn liveness_action(&mut self) -> LivenessAction {
        self.liveness.poll_at(Instant::now())
    }

    pub fn request_timeouts(&mut self) -> Vec<PeerEvent> {
        let peer_id = self.peer.peer_id;
        self.requests
            .expire()
            .into_iter()
            .map(|key| {
                self.score.apply(crate::Misbehavior::SyncTimeout);
                PeerEvent::RequestTimedOut { peer_id, key }
            })
            .collect()
    }

    pub async fn send(&mut self, message: &Message, class: QueueClass) -> Result<bool, PeerError> {
        let (_, payload) = encode_message(message)?;
        let bytes = FRAME_HEADER_BYTES
            .checked_add(payload.len())
            .ok_or(PeerError::QueueItemTooLarge)?;
        let Some(_permit) = self.budget.reserve(class, bytes).await? else {
            return Ok(false);
        };
        self.connection.write_message(message).await?;
        Ok(true)
    }

    pub async fn read_event(&mut self) -> Result<PeerEvent, PeerError> {
        loop {
            let message = self.connection.read_message().await?;
            let now = Instant::now();
            self.liveness.note_activity(now);
            if let Message::Pong(nonce) = message {
                self.liveness.on_pong(nonce, now);
                return Ok(PeerEvent::Message {
                    peer_id: self.peer.peer_id,
                    message: Message::Pong(nonce),
                });
            }
            if let Some(event) = self.classify_message_at(message, now) {
                return Ok(event);
            }
        }
    }

    pub async fn shutdown(&mut self) -> Result<(), PeerError> {
        self.connection.shutdown().await.map_err(PeerError::from)
    }

    pub(crate) fn classify_message_at(
        &mut self,
        message: Message,
        now: Instant,
    ) -> Option<PeerEvent> {
        let peer_id = self.peer.peer_id;
        let key = match &message {
            Message::Headers(_) => Some(RequestKey::Headers),
            Message::Transaction(transaction) => Some(RequestKey::Object(InventoryItem {
                kind: InventoryKind::Transaction,
                hash: transaction.txid(),
            })),
            Message::Block(block) => Some(RequestKey::Object(InventoryItem {
                kind: InventoryKind::Block,
                hash: block.header.block_id(),
            })),
            _ => None,
        };

        let Some(key) = key else {
            return Some(PeerEvent::Message { peer_id, message });
        };
        match self.requests.classify_key_at(key, now) {
            ResponseDisposition::Matched(key) => Some(PeerEvent::MatchedResponse {
                peer_id,
                key,
                message,
            }),
            ResponseDisposition::GraceDrop(_) => None,
            ResponseDisposition::Unsolicited(_) => {
                self.score.apply(crate::Misbehavior::UnsolicitedObject);
                Some(PeerEvent::Unsolicited { peer_id, message })
            }
        }
    }
}
