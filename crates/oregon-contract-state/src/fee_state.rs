use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;

use crate::{StateError, StateWrite, StateWriteSet};

pub const FEE_HEIGHT_KEY_V1: &[u8] = b"fee/v1/height";
pub const FEE_BASE_FEE_PER_WEIGHT_KEY_V1: &[u8] = b"fee/v1/base_fee_per_weight";
pub const FEE_BLOCK_WEIGHT_USED_KEY_V1: &[u8] = b"fee/v1/block_weight_used";
pub const FEE_EXECUTION_FEE_TOTAL_KEY_V1: &[u8] = b"fee/v1/execution_fee_total";
pub const FEE_PRODUCER_COINBASE_TXID_KEY_V1: &[u8] = b"fee/v1/producer_coinbase_txid";
pub const FEE_RESERVE_TRANSITION_ID_KEY_V1: &[u8] = b"fee/v1/reserve_transition_id";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeStateValuesV1 {
    pub height: u64,
    pub base_fee_per_weight: u64,
    pub block_weight_used: u64,
    pub execution_fee_total: u64,
    pub producer_coinbase_txid: Hash256,
    pub reserve_transition_id: Hash256,
}

impl FeeStateValuesV1 {
    pub fn write_set(&self) -> Result<StateWriteSet, StateError> {
        StateWriteSet::new(
            CommitmentDomainId::FeeState,
            vec![
                StateWrite::put(
                    FEE_HEIGHT_KEY_V1.to_vec(),
                    self.height.to_le_bytes().to_vec(),
                ),
                StateWrite::put(
                    FEE_BASE_FEE_PER_WEIGHT_KEY_V1.to_vec(),
                    self.base_fee_per_weight.to_le_bytes().to_vec(),
                ),
                StateWrite::put(
                    FEE_BLOCK_WEIGHT_USED_KEY_V1.to_vec(),
                    self.block_weight_used.to_le_bytes().to_vec(),
                ),
                StateWrite::put(
                    FEE_EXECUTION_FEE_TOTAL_KEY_V1.to_vec(),
                    self.execution_fee_total.to_le_bytes().to_vec(),
                ),
                StateWrite::put(
                    FEE_PRODUCER_COINBASE_TXID_KEY_V1.to_vec(),
                    self.producer_coinbase_txid.as_bytes().to_vec(),
                ),
                StateWrite::put(
                    FEE_RESERVE_TRANSITION_ID_KEY_V1.to_vec(),
                    self.reserve_transition_id.as_bytes().to_vec(),
                ),
            ],
        )
    }
}
