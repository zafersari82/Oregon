#![forbid(unsafe_code)]

mod branch;
mod config;
mod error;
mod reorg;
mod state;

pub use config::ChainConfig;
pub use error::ChainStateError;
pub use state::{AcceptOutcome, ChainState, SessionHealth, Tip};

#[cfg(test)]
mod task7_storage_fault_tests;
#[cfg(test)]
mod tests;
