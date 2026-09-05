#![forbid(unsafe_code)]

mod budget;
mod config;
mod cooldown;
mod error;
mod handshake;
mod request;
mod score;
mod service;
mod session;

pub use config::{
    CONTROL_RESERVED_BYTES, CONTROL_RESERVED_FRAMES, DEFAULT_MAX_INBOUND, DEFAULT_MAX_OUTBOUND,
    DEFAULT_MAX_PEERS, Direction, HANDSHAKE_TIMEOUT, HARD_MAX_PEERS, MAX_PENDING_HANDSHAKES,
    MAX_QUEUE_BYTES_GLOBAL, MAX_QUEUE_BYTES_PEER, MAX_QUEUE_FRAMES_PEER, PeerConfig,
    QUEUE_ENQUEUE_TIMEOUT, QueueClass,
};
pub use cooldown::{CooldownTable, DISCONNECT_COOLDOWN, MAX_COOLDOWN_ENTRIES, canonical_ip};
pub use error::PeerError;
pub use handshake::{HandshakeState, generate_process_nonce, preferred_direction};
pub use request::{
    EXPIRED_REQUEST_GRACE, MAX_RECENTLY_EXPIRED_REQUESTS_PER_PEER, PerformanceSnapshot,
    RESPONSE_START_TIMEOUT, RequestError, RequestKey, RequestRegistry, ResponseDisposition,
};
pub use score::{Misbehavior, PeerFeedback, PeerScore, ScoreDecision};
pub use service::{
    EstablishOutcome, IDLE_TIMEOUT, LivenessAction, LivenessState, PING_INTERVAL, PONG_TIMEOUT,
    PeerCommand, PeerService,
};
pub use session::{DisconnectReason, EstablishedPeer, PeerEvent, PeerId, PeerSession};

#[cfg(test)]
mod request_policy_tests;
#[cfg(test)]
mod tests;
