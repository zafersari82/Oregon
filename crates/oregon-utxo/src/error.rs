use oregon_consensus::ConsensusError;
use oregon_primitives::OutPoint;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UtxoError {
    #[error("missing UTXO: {0:?}")]
    MissingUtxo(OutPoint),
    #[error("duplicate input: {0:?}")]
    DuplicateInput(OutPoint),
    #[error("duplicate persisted UTXO outpoint: {0:?}")]
    DuplicatePersistedOutpoint(OutPoint),
    #[error("coinbase output is immature")]
    ImmatureCoinbase,
    #[error("transaction outputs exceed inputs")]
    OutputValueExceedsInput,
    #[error("amount arithmetic overflow")]
    AmountOverflow,
    #[error("transaction output index exceeds u32")]
    OutputIndexOverflow,
    #[error("output collides with an existing UTXO: {0:?}")]
    OutputCollision(OutPoint),
    #[error("spend authorization failed")]
    SpendAuthorizationFailed,
    #[error("transaction order is invalid for current UTXO state")]
    InvalidBlockOrder,
    #[error("block undo does not match current state")]
    UndoMismatch,
    #[error(transparent)]
    Consensus(#[from] ConsensusError),
}
