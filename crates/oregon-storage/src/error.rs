use thiserror::Error;

use crate::schema::SchemaVersion;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("rocksdb error: {0}")]
    RocksDb(#[from] rocksdb::Error),
    #[error("corrupt storage data: {0}")]
    CorruptData(String),
    #[error("unsupported storage schema: {0:?}")]
    UnsupportedSchema(SchemaVersion),
    #[error("durability failure: {0}")]
    DurabilityFailure(String),
}
