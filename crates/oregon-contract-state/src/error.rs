use oregon_primitives::Hash256;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StateError {
    #[error("state key length {0} exceeds the structural limit")]
    KeyTooLarge(usize),
    #[error("state value length {0} exceeds the structural limit")]
    ValueTooLarge(usize),
    #[error("SMT depth {0} is out of range")]
    DepthOutOfRange(usize),
    #[error("explicit branch with two default children is non-canonical at depth {0}")]
    NonCanonicalEmptyBranch(u16),
    #[error("missing non-empty state node {0}")]
    MissingNode(Hash256),
    #[error("state node hash mismatch for {0}")]
    NodeHashMismatch(Hash256),
    #[error("state node depth mismatch: expected {expected}, got {actual}")]
    NodeDepthMismatch { expected: u16, actual: u16 },
    #[error("unexpected leaf before depth 256")]
    UnexpectedLeaf,
    #[error("unexpected branch at leaf depth")]
    UnexpectedBranch,
    #[error("missing state value {0}")]
    MissingValue(Hash256),
    #[error("state value hash mismatch for {0}")]
    ValueHashMismatch(Hash256),
    #[error("duplicate or colliding state path {0}")]
    DuplicatePath(Hash256),
    #[error("state write set has {0} entries, exceeding the limit")]
    WriteSetTooLarge(usize),
    #[error("state proof length {0} exceeds the limit")]
    ProofTooLarge(usize),
    #[error("malformed sparse Merkle proof")]
    MalformedProof,
    #[error("proof redundantly encodes a default sibling at depth {0}")]
    RedundantDefaultSibling(usize),
    #[error("sparse Merkle proof does not match the expected root")]
    InvalidProof,
    #[error("state domain does not match the snapshot/write set")]
    DomainMismatch,
}
