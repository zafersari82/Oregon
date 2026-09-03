use oregon_consensus::ConsensusError;
use oregon_pow::PowError;
use oregon_primitives::Hash256;
use oregon_storage::StorageError;
use oregon_utxo::UtxoError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChainStateError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("consensus error: {0}")]
    Consensus(#[from] ConsensusError),
    #[error("proof-of-work engine error: {0}")]
    Pow(#[from] PowError),
    #[error("utxo reconstruction error: {0}")]
    Utxo(#[from] UtxoError),
    #[error("unknown candidate parent: {0:?}")]
    UnknownParent(Hash256),
    #[error("missing retained block body required for reorg: {0:?}")]
    MissingBlockBody(Hash256),
    #[error("missing retained undo required for reorg: {0:?}")]
    MissingUndo(Hash256),
    #[error("chain configuration mismatch: {0}")]
    ConfigMismatch(String),
    #[error("corrupt persistent chainstate: {0}")]
    CorruptState(String),
    #[error("candidate transition is deferred to a later M4 stage: {0}")]
    DeferredTransition(&'static str),
    #[error("chainstate session is storage-faulted")]
    StorageFaulted,
    #[error("reindex required")]
    ReindexRequired,
}
