#![forbid(unsafe_code)]

mod admission;
mod branch;
mod config;
mod error;
mod header;
mod prune;
mod recovery;
mod reorg;
mod state;
mod transition;
mod utxo_delta;

pub use config::ChainConfig;
pub use error::ChainStateError;
pub use header::{HeaderImportOutcome, HeaderImportStatus, HeaderTip};
pub use prune::PruneReport;
pub use state::{AcceptOutcome, ChainState, SessionHealth, Tip};

#[cfg(test)]
mod body_promotion_tests;
#[cfg(test)]
mod header_contract_tests;
#[cfg(test)]
mod header_import_tests;
#[cfg(test)]
mod recovery_acceptance_tests;
#[cfg(test)]
mod storage_fault_tests;
#[cfg(test)]
mod sync_view_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
