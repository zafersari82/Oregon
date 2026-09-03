use oregon_primitives::OutPoint;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum UtxoError {
    #[error("missing UTXO: {0:?}")]
    MissingUtxo(OutPoint),
    #[error("duplicate input: {0:?}")]
    DuplicateInput(OutPoint),
    #[error("coinbase output is immature")]
    ImmatureCoinbase,
    #[error("transaction outputs exceed inputs")]
    OutputValueExceedsInput,
    #[error("amount arithmetic overflow")]
    AmountOverflow,
    #[error("transaction output index exceeds u32")]
    OutputIndexOverflow,
    #[error("spend authorization failed")]
    SpendAuthorizationFailed,
    #[error("transaction order is invalid for current UTXO state")]
    InvalidBlockOrder,
    #[error("block undo does not match current state")]
    UndoMismatch,
}
