use oregon_storage::StorageError;
use oregon_utxo::UtxoError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChainStateError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("utxo reconstruction error: {0}")]
    Utxo(#[from] UtxoError),
    #[error("chain configuration mismatch: {0}")]
    ConfigMismatch(String),
    #[error("corrupt persistent chainstate: {0}")]
    CorruptState(String),
    #[error("reindex required")]
    ReindexRequired,
}
