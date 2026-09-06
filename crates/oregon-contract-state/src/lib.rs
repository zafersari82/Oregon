#![forbid(unsafe_code)]

mod error;
mod execution_accounting;
mod fee_state;
mod hash;
mod node;
mod proof;
mod source;
mod transition;

pub use error::StateError;
pub use execution_accounting::{
    ExecutionAccountingWritesV1, balance_key, decode_accounting_u64, encode_accounting_u64,
    encode_reserve_outpoint, reserve_outpoint_key, sequence_key, total_execution_balance_key,
};
pub use fee_state::{
    FEE_BASE_FEE_PER_WEIGHT_KEY_V1, FEE_BLOCK_WEIGHT_USED_KEY_V1, FEE_EXECUTION_FEE_TOTAL_KEY_V1,
    FEE_HEIGHT_KEY_V1, FEE_PRODUCER_COINBASE_TXID_KEY_V1, FEE_RESERVE_TRANSITION_ID_KEY_V1,
    FeeStateValuesV1,
};
pub use hash::{
    MAX_STATE_KEY_BYTES, MAX_STATE_VALUE_BYTES, SMT_DEPTH, branch_hash, empty_hashes, leaf_hash,
    path_bit, path_key, value_hash,
};
pub use node::StateNode;
pub use proof::{
    MAX_SMT_PROOF_BYTES, MAX_SMT_SIBLINGS, SMT_PROOF_BITMAP_BYTES, SMT_PROOF_VERSION,
    SparseMerkleProofV1, prove, verify_proof,
};
pub use source::StateSource;
pub use transition::{
    DomainSnapshot, MAX_STATE_WRITE_SET_ENTRIES, StateTransition, StateWrite, StateWriteSet,
    apply_write_set, read_value,
};
