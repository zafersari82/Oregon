#![forbid(unsafe_code)]

mod db;
mod error;
mod schema;

pub use db::{CF_BLOCK_INDEX, CF_BLOCKS, CF_CHAIN_META, CF_UNDO, CF_UTXO, OregonDb};
pub use error::StorageError;
pub use schema::{SCHEMA_VERSION, SchemaVersion};

#[cfg(test)]
mod tests;
