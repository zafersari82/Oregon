#![forbid(unsafe_code)]

mod batch;
mod codec;
mod db;
mod error;
mod records;
mod schema;

pub use batch::{DurabilityMode, StorageBatch};
pub use codec::{
    decode_block_undo, decode_outpoint_key, decode_utxo_entry, encode_block_undo,
    encode_outpoint_key, encode_utxo_entry,
};
#[cfg(any(test, feature = "test-hooks"))]
pub use db::TestHooks;
pub use db::{CF_BLOCK_INDEX, CF_BLOCKS, CF_CHAIN_META, CF_UNDO, CF_UTXO, OregonDb};
pub use error::StorageError;
pub use records::{
    ACTIVE_TIP_HEIGHT_KEY, ACTIVE_TIP_ID_KEY, BlockIndexRecord, CONFIG_ANCHOR_ID_KEY,
    CONFIG_GENESIS_TIMESTAMP_KEY, HEALTH_STATE_KEY, NodeHealth, PRUNE_CURSOR_KEY,
    SCHEMA_MIGRATION_KEY, ValidationStatus, active_height_key, decode_block_index,
    decode_node_health, encode_block_index, encode_node_health,
};
pub use schema::{SCHEMA_VERSION, SchemaVersion};

#[cfg(test)]
mod batch_tests;
#[cfg(test)]
mod migration_tests;
#[cfg(test)]
mod recovery_acceptance_tests;
#[cfg(test)]
mod tests;
