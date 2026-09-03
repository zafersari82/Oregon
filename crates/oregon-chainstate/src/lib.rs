#![forbid(unsafe_code)]

mod config;
mod error;
mod state;

pub use config::ChainConfig;
pub use error::ChainStateError;
pub use state::{ChainState, SessionHealth, Tip};

#[cfg(test)]
mod tests;
