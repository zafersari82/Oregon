use std::time::Duration;

use crate::PeerError;

pub const DEFAULT_MAX_PEERS: usize = 64;
pub const DEFAULT_MAX_OUTBOUND: usize = 16;
pub const DEFAULT_MAX_INBOUND: usize = 48;
pub const HARD_MAX_PEERS: usize = 128;

pub const MAX_QUEUE_FRAMES_PEER: usize = 256;
pub const MAX_QUEUE_BYTES_PEER: usize = 4 * 1024 * 1024;
pub const MAX_QUEUE_BYTES_GLOBAL: usize = 64 * 1024 * 1024;
pub const CONTROL_RESERVED_FRAMES: usize = 16;
pub const CONTROL_RESERVED_BYTES: usize = 64 * 1024;
pub const QUEUE_ENQUEUE_TIMEOUT: Duration = Duration::from_secs(2);

pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
pub const MAX_PENDING_HANDSHAKES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueueClass {
    Control,
    RequiredData,
    Gossip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerConfig {
    pub max_peers: usize,
    pub max_outbound: usize,
    pub max_inbound: usize,
}

impl PeerConfig {
    pub fn new(
        max_peers: usize,
        max_outbound: usize,
        max_inbound: usize,
    ) -> Result<Self, PeerError> {
        if max_peers == 0 {
            return Err(PeerError::InvalidConfig("max_peers must be nonzero"));
        }
        if max_peers > HARD_MAX_PEERS {
            return Err(PeerError::InvalidConfig("max_peers exceeds hard limit"));
        }
        let directional = max_outbound
            .checked_add(max_inbound)
            .ok_or(PeerError::InvalidConfig("peer limit sum overflow"))?;
        if directional > max_peers {
            return Err(PeerError::InvalidConfig(
                "inbound + outbound exceeds max_peers",
            ));
        }
        Ok(Self {
            max_peers,
            max_outbound,
            max_inbound,
        })
    }
}

impl Default for PeerConfig {
    fn default() -> Self {
        Self {
            max_peers: DEFAULT_MAX_PEERS,
            max_outbound: DEFAULT_MAX_OUTBOUND,
            max_inbound: DEFAULT_MAX_INBOUND,
        }
    }
}
