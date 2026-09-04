#![forbid(unsafe_code)]

mod batch;
mod codec;
mod db;
mod error;
mod records;
mod schema;

pub use batch::{DurabilityMode, StorageBatch};
pub use db::OregonDb;
#[cfg(any(test, feature = "test-hooks"))]
pub use db::TestHooks;
pub use error::StorageError;
pub use records::{BlockIndexRecord, NodeHealth, ValidationStatus};
pub use schema::SchemaVersion;

#[cfg(test)]
mod batch_tests;
#[cfg(test)]
mod migration_tests;
#[cfg(test)]
mod recovery_acceptance_tests;
#[cfg(test)]
mod tests;
