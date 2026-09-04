#![forbid(unsafe_code)]

mod admission;
mod branch;
mod config;
mod error;
mod prune;
mod recovery;
mod reorg;
mod state;
mod transition;
mod utxo_delta;

pub use config::ChainConfig;
pub use error::ChainStateError;
pub use prune::PruneReport;
pub use state::{AcceptOutcome, ChainState, SessionHealth, Tip};

#[cfg(test)]
mod recovery_acceptance_tests;
#[cfg(test)]
mod storage_fault_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
