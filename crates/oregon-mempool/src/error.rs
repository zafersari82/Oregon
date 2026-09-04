use oregon_consensus::NormalTransactionError;
use oregon_primitives::{Hash256, OutPoint};
use oregon_utxo::UtxoError;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MempoolError {
    #[error("invalid mempool configuration")]
    InvalidConfig,
    #[error("chain height overflow")]
    HeightOverflow,
    #[error("transaction already known: {0}")]
    AlreadyKnown(Hash256),
    #[error("mempool chain context is stale")]
    StaleChainContext,
    #[error("mempool conflict on {outpoint:?}; existing transaction {existing_txid}")]
    Conflict {
        outpoint: OutPoint,
        existing_txid: Hash256,
    },
    #[error("missing transaction dependency: {0:?}")]
    MissingDependency(OutPoint),
    #[error("parent output does not exist: {0:?}")]
    InvalidParentOutput(OutPoint),
    #[error("too many unconfirmed ancestors")]
    TooManyAncestors,
    #[error("too many unconfirmed descendants")]
    TooManyDescendants,
    #[error("mempool capacity rejected transaction")]
    CapacityRejected,
    #[error("mempool dependency cycle")]
    DependencyCycle,
    #[error("mempool invariant violation")]
    InvariantViolation,
    #[error(transparent)]
    Structural(#[from] NormalTransactionError),
    #[error(transparent)]
    Utxo(#[from] UtxoError),
}
