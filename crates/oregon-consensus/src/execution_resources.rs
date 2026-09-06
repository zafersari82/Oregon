use oregon_primitives::MAX_SUPPLY_BASE_UNITS;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ResourceFeeError {
    #[error("unsupported resource parameter version {0}")]
    UnsupportedVersion(u16),
    #[error("invalid resource fee parameters")]
    InvalidParameters,
    #[error("parent base fee is outside configured bounds")]
    ParentFeeOutOfRange,
    #[error("parent weight exceeds the block limit")]
    ParentWeightExceeded,
    #[error("transaction weight is invalid")]
    InvalidTransactionWeight,
    #[error("block weight limit would be exceeded")]
    BlockWeightExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeParametersV1 {
    target_weight: u64,
    max_transaction_weight: u64,
    adjustment_denominator: u64,
    min_base_fee: u64,
    max_base_fee: u64,
}

impl FeeParametersV1 {
    pub fn new(
        version: u16,
        target_weight: u64,
        max_transaction_weight: u64,
        adjustment_denominator: u64,
        min_base_fee: u64,
        max_base_fee: u64,
    ) -> Result<Self, ResourceFeeError> {
        if version != 1 {
            return Err(ResourceFeeError::UnsupportedVersion(version));
        }
        let Some(block_weight_limit) = target_weight.checked_mul(2) else {
            return Err(ResourceFeeError::InvalidParameters);
        };
        if target_weight == 0
            || max_transaction_weight == 0
            || max_transaction_weight > block_weight_limit
            || adjustment_denominator < 2
            || min_base_fee == 0
            || min_base_fee > max_base_fee
            || max_base_fee > MAX_SUPPLY_BASE_UNITS
        {
            return Err(ResourceFeeError::InvalidParameters);
        }
        Ok(Self {
            target_weight,
            max_transaction_weight,
            adjustment_denominator,
            min_base_fee,
            max_base_fee,
        })
    }

    pub const fn target_weight(self) -> u64 {
        self.target_weight
    }

    pub const fn max_transaction_weight(self) -> u64 {
        self.max_transaction_weight
    }

    pub const fn adjustment_denominator(self) -> u64 {
        self.adjustment_denominator
    }

    pub const fn min_base_fee(self) -> u64 {
        self.min_base_fee
    }

    pub const fn max_base_fee(self) -> u64 {
        self.max_base_fee
    }

    pub const fn block_weight_limit(self) -> u64 {
        self.target_weight * 2
    }
}

pub fn next_base_fee(
    parameters: &FeeParametersV1,
    parent_base_fee: u64,
    parent_weight: u64,
) -> Result<u64, ResourceFeeError> {
    if !(parameters.min_base_fee..=parameters.max_base_fee).contains(&parent_base_fee) {
        return Err(ResourceFeeError::ParentFeeOutOfRange);
    }
    if parent_weight > parameters.block_weight_limit() {
        return Err(ResourceFeeError::ParentWeightExceeded);
    }
    if parent_weight == parameters.target_weight {
        return Ok(parent_base_fee);
    }

    let distance = u128::from(parent_weight.abs_diff(parameters.target_weight));
    let denominator =
        u128::from(parameters.target_weight) * u128::from(parameters.adjustment_denominator);
    let change = (u128::from(parent_base_fee) * distance / denominator) as u64;
    if parent_weight > parameters.target_weight {
        Ok(parent_base_fee
            .saturating_add(change.max(1))
            .min(parameters.max_base_fee))
    } else {
        Ok(parent_base_fee
            .saturating_sub(change)
            .max(parameters.min_base_fee))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockWeightBudget {
    limit: u64,
    transaction_limit: u64,
    consumed: u64,
}

impl BlockWeightBudget {
    pub const fn new(parameters: &FeeParametersV1) -> Self {
        Self {
            limit: parameters.block_weight_limit(),
            transaction_limit: parameters.max_transaction_weight(),
            consumed: 0,
        }
    }

    pub fn include(&mut self, actual_weight: u64) -> Result<(), ResourceFeeError> {
        if actual_weight == 0 || actual_weight > self.transaction_limit {
            return Err(ResourceFeeError::InvalidTransactionWeight);
        }
        let Some(next) = self.consumed.checked_add(actual_weight) else {
            return Err(ResourceFeeError::BlockWeightExceeded);
        };
        if next > self.limit {
            return Err(ResourceFeeError::BlockWeightExceeded);
        }
        self.consumed = next;
        Ok(())
    }

    pub const fn consumed(self) -> u64 {
        self.consumed
    }

    pub const fn remaining(self) -> u64 {
        self.limit - self.consumed
    }
}
