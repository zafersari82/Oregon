use oregon_network::NetworkError;
use oregon_protocol::ProtocolError;
use thiserror::Error;

use crate::Direction;

#[derive(Debug, Error)]
pub enum PeerError {
    #[error("invalid peer configuration: {0}")]
    InvalidConfig(&'static str),
    #[error("pending handshake limit reached")]
    PendingHandshakeLimit,
    #[error("peer limit reached")]
    PeerLimit,
    #[error("{0:?} peer limit reached")]
    DirectionLimit(Direction),
    #[error("peer handshake exceeded 10 seconds")]
    HandshakeTimeout,
    #[error("peer handshake violation: {0}")]
    HandshakeViolation(&'static str),
    #[error("peer announced a different Oregon chain id")]
    WrongChain,
    #[error("connection resolves to this process nonce")]
    SelfPeer,
    #[error("duplicate peer connection lost deterministic arbitration")]
    DuplicatePeer,
    #[error("HelloAck does not match locally negotiated parameters")]
    AckMismatch,
    #[error("peer queue could not make progress within 2 seconds")]
    QueueEnqueueTimeout,
    #[error("peer queue item cannot fit within the configured bound")]
    QueueItemTooLarge,
    #[error("operating-system entropy source failed")]
    Entropy,
    #[error("peer identifier space exhausted")]
    PeerIdExhausted,
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error(transparent)]
    Network(#[from] NetworkError),
}

impl PartialEq for PeerError {
    fn eq(&self, other: &Self) -> bool {
        use PeerError::*;
        match (self, other) {
            (InvalidConfig(a), InvalidConfig(b)) => a == b,
            (PendingHandshakeLimit, PendingHandshakeLimit)
            | (PeerLimit, PeerLimit)
            | (HandshakeTimeout, HandshakeTimeout)
            | (WrongChain, WrongChain)
            | (SelfPeer, SelfPeer)
            | (DuplicatePeer, DuplicatePeer)
            | (AckMismatch, AckMismatch)
            | (QueueEnqueueTimeout, QueueEnqueueTimeout)
            | (QueueItemTooLarge, QueueItemTooLarge)
            | (Entropy, Entropy)
            | (PeerIdExhausted, PeerIdExhausted) => true,
            (DirectionLimit(a), DirectionLimit(b)) => a == b,
            (HandshakeViolation(a), HandshakeViolation(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for PeerError {}
