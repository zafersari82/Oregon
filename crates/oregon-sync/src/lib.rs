#![forbid(unsafe_code)]

mod error;
mod locator;
mod scheduler;
mod state;
mod view;

pub use error::{SyncError, SyncViewError};
pub use locator::{
    build_locator, headers_after_common_height, highest_locator_hit, locator_heights,
    validate_headers_response,
};
pub use scheduler::{
    BlockScheduler, MAX_BLOCK_ATTEMPTS, MAX_BUFFERED_BLOCKS, MAX_IN_FLIGHT_BLOCKS_GLOBAL,
    MAX_IN_FLIGHT_BLOCKS_PEER, SyncPeer,
};
pub use state::SyncAction;
pub use view::{ChainSyncView, SyncTip};

#[cfg(test)]
mod tests;
