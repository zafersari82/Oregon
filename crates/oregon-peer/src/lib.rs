#![forbid(unsafe_code)]

mod budget;
mod config;
mod error;
mod handshake;
mod service;
mod session;

pub use config::{
    CONTROL_RESERVED_BYTES, CONTROL_RESERVED_FRAMES, DEFAULT_MAX_INBOUND, DEFAULT_MAX_OUTBOUND,
    DEFAULT_MAX_PEERS, Direction, HANDSHAKE_TIMEOUT, HARD_MAX_PEERS, MAX_PENDING_HANDSHAKES,
    MAX_QUEUE_BYTES_GLOBAL, MAX_QUEUE_BYTES_PEER, MAX_QUEUE_FRAMES_PEER, PeerConfig,
    QUEUE_ENQUEUE_TIMEOUT, QueueClass,
};
pub use error::PeerError;
pub use handshake::{HandshakeState, generate_process_nonce, preferred_direction};
pub use service::{EstablishOutcome, PeerService};
pub use session::{DisconnectReason, EstablishedPeer, PeerEvent, PeerId, PeerSession};

#[cfg(test)]
mod request_policy_tests;
#[cfg(test)]
mod tests;
