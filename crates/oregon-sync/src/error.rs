use oregon_peer::PeerId;
use oregon_primitives::Hash256;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SyncViewError {
    #[error("authoritative chain synchronization view is unavailable")]
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SyncError {
    #[error("synchronization target plan contains duplicate block {0}")]
    DuplicateTarget(Hash256),
    #[error("received block {block_id} without matching in-flight ownership for peer {peer_id:?}")]
    UnexpectedBlock { peer_id: PeerId, block_id: Hash256 },
    #[error("buffered block cap reached")]
    BufferFull,
    #[error("header response does not attach to the selected common ancestor")]
    DetachedHeaders,
    #[error("header response is not contiguous")]
    NonContiguousHeaders,
    #[error("header response exceeds protocol limit")]
    TooManyHeaders,
    #[error(transparent)]
    View(#[from] SyncViewError),
}
