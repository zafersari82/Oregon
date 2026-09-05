#![forbid(unsafe_code)]

pub mod amount;
pub mod block;
pub mod encoding;
pub mod error;
pub mod execution_address;
pub mod execution_envelope;
pub mod hash;
pub mod merkle;
pub mod state_commitment;
pub mod transaction;

pub use amount::{
    Amount, BASE_UNITS_PER_OREG, FOUNDER_ALLOCATION_BASE_UNITS, MAX_SUPPLY_BASE_UNITS,
};
pub use block::{Block, BlockHeader};
pub use encoding::{DecodeLimits, Decoder, write_varint};
pub use error::PrimitiveError;
pub use hash::{Hash256, domain_hash};
pub use merkle::transaction_root;
pub use transaction::{OutPoint, Transaction, TxInput, TxOutput};
