#![forbid(unsafe_code)]

mod error;
mod hash;
mod node;
mod transition;

pub use error::StateError;
pub use hash::{
    MAX_STATE_KEY_BYTES, MAX_STATE_VALUE_BYTES, SMT_DEPTH, branch_hash, empty_hashes, leaf_hash,
    path_bit, path_key, value_hash,
};
pub use node::StateNode;
pub use transition::{
    MAX_STATE_WRITE_SET_ENTRIES, DomainSnapshot, StateSource, StateTransition, StateWrite,
    StateWriteSet, apply_write_set, read_value,
};
