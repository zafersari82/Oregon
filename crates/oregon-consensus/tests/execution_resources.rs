use oregon_consensus::execution_resources::{
    BlockWeightBudget, FeeParametersV1, ResourceFeeError, next_base_fee,
};
use oregon_primitives::MAX_SUPPLY_BASE_UNITS;
use proptest::prelude::*;

fn parameters() -> FeeParametersV1 {
    FeeParametersV1::new(1, 100, 120, 8, 1, 1_000).unwrap()
}

#[test]
fn validated_parameters_expose_derived_limits() {
    let parameters = parameters();
    assert_eq!(parameters.target_weight(), 100);
    assert_eq!(parameters.max_transaction_weight(), 120);
    assert_eq!(parameters.adjustment_denominator(), 8);
    assert_eq!(parameters.min_base_fee(), 1);
    assert_eq!(parameters.max_base_fee(), 1_000);
    assert_eq!(parameters.block_weight_limit(), 200);
}

#[test]
fn malformed_versions_and_parameters_fail_closed() {
    assert_eq!(
        FeeParametersV1::new(0, 100, 100, 8, 1, 1_000),
        Err(ResourceFeeError::UnsupportedVersion(0))
    );
    assert_eq!(
        FeeParametersV1::new(2, 100, 100, 8, 1, 1_000),
        Err(ResourceFeeError::UnsupportedVersion(2))
    );
    for result in [
        FeeParametersV1::new(1, 0, 1, 8, 1, 1_000),
        FeeParametersV1::new(1, u64::MAX / 2 + 1, 1, 8, 1, 1_000),
        FeeParametersV1::new(1, 100, 0, 8, 1, 1_000),
        FeeParametersV1::new(1, 100, 201, 8, 1, 1_000),
        FeeParametersV1::new(1, 100, 100, 1, 1, 1_000),
        FeeParametersV1::new(1, 100, 100, 8, 0, 1_000),
        FeeParametersV1::new(1, 100, 100, 8, 11, 10),
        FeeParametersV1::new(1, 100, 100, 8, 1, MAX_SUPPLY_BASE_UNITS + 1),
    ] {
        assert_eq!(result, Err(ResourceFeeError::InvalidParameters));
    }
}

#[test]
fn empty_parent_lowers_fee() {
    let parameters = parameters();
    for (used, want) in [(0, 88), (99, 100), (100, 100), (101, 101), (200, 112)] {
        assert_eq!(next_base_fee(&parameters, 100, used), Ok(want));
    }
}

#[test]
fn tiny_upward_change_costs_one_unit() {
    assert_eq!(next_base_fee(&parameters(), 1, 101), Ok(2));
}

#[test]
fn fee_never_exceeds_ceiling() {
    let parameters = FeeParametersV1::new(1, 100, 100, 8, 1, 101).unwrap();
    let mut fee = 100;
    for _ in 0..100 {
        fee = next_base_fee(&parameters, fee, 200).unwrap();
        assert!(fee <= 101);
    }
    assert_eq!(fee, 101);
}

#[test]
fn fee_respects_configured_floor() {
    let parameters = FeeParametersV1::new(1, 100, 100, 2, 60, 1_000).unwrap();
    assert_eq!(next_base_fee(&parameters, 100, 0), Ok(60));
    assert_eq!(next_base_fee(&parameters, 60, 0), Ok(60));
}

#[test]
fn invalid_parent_utilization_rejected() {
    let parameters = parameters();
    assert_eq!(
        next_base_fee(&parameters, 100, 201),
        Err(ResourceFeeError::ParentWeightExceeded)
    );
    assert_eq!(
        next_base_fee(&parameters, 0, 100),
        Err(ResourceFeeError::ParentFeeOutOfRange)
    );
    assert_eq!(
        next_base_fee(&parameters, 1_001, 100),
        Err(ResourceFeeError::ParentFeeOutOfRange)
    );
}

#[test]
fn fee_arithmetic_uses_wide_products() {
    let target = u64::MAX / 2;
    let parameters = FeeParametersV1::new(1, target, target, 2, 1, MAX_SUPPLY_BASE_UNITS).unwrap();
    assert_eq!(
        next_base_fee(&parameters, MAX_SUPPLY_BASE_UNITS, 0),
        Ok(MAX_SUPPLY_BASE_UNITS / 2)
    );
}

#[test]
fn producer_sequences_match_reference() {
    let parameters = parameters();
    let mut full = 100;
    let mut full_sequence = Vec::new();
    let mut empty = 100;
    let mut empty_sequence = Vec::new();
    for _ in 0..5 {
        full = next_base_fee(&parameters, full, 200).unwrap();
        full_sequence.push(full);
        empty = next_base_fee(&parameters, empty, 0).unwrap();
        empty_sequence.push(empty);
    }
    assert_eq!(full_sequence, [112, 126, 141, 158, 177]);
    assert_eq!(empty_sequence, [88, 77, 68, 60, 53]);
}

#[test]
fn block_budget_rejects_transaction_and_block_overruns_atomically() {
    let mut budget = BlockWeightBudget::new(&parameters());
    assert_eq!(budget.consumed(), 0);
    assert_eq!(budget.remaining(), 200);
    assert_eq!(
        budget.include(0),
        Err(ResourceFeeError::InvalidTransactionWeight)
    );
    assert_eq!(budget.consumed(), 0);
    budget.include(120).unwrap();
    assert_eq!(budget.consumed(), 120);
    assert_eq!(
        budget.include(121),
        Err(ResourceFeeError::InvalidTransactionWeight)
    );
    assert_eq!(budget.consumed(), 120);
    budget.include(80).unwrap();
    assert_eq!(budget.consumed(), 200);
    assert_eq!(budget.remaining(), 0);
    assert_eq!(
        budget.include(1),
        Err(ResourceFeeError::BlockWeightExceeded)
    );
    assert_eq!(budget.consumed(), 200);
}

#[test]
fn block_budget_rejects_accumulation_overflow_atomically() {
    let target = u64::MAX / 2;
    let parameters = FeeParametersV1::new(1, target, u64::MAX - 1, 8, 1, 1_000).unwrap();
    let mut budget = BlockWeightBudget::new(&parameters);
    budget.include(u64::MAX - 1).unwrap();
    assert_eq!(
        budget.include(u64::MAX - 1),
        Err(ResourceFeeError::BlockWeightExceeded)
    );
    assert_eq!(budget.consumed(), u64::MAX - 1);
}

proptest! {
    #[test]
    fn fee_update_obeys_direction_range_and_rate_bounds(
        target in 1u64..=1_000_000,
        denominator in 2u64..=100,
        parent_fee in 1u64..=1_000_000_000_000,
        utilization_factor in 0u8..=2,
    ) {
        let parameters = FeeParametersV1::new(
            1,
            target,
            target,
            denominator,
            1,
            MAX_SUPPLY_BASE_UNITS,
        ).unwrap();
        let parent_weight = match utilization_factor {
            0 => 0,
            1 => target,
            _ => target * 2,
        };
        let next = next_base_fee(&parameters, parent_fee, parent_weight).unwrap();
        prop_assert!((1..=MAX_SUPPLY_BASE_UNITS).contains(&next));
        match utilization_factor {
            0 => {
                prop_assert!(next <= parent_fee);
                prop_assert!(parent_fee - next <= parent_fee / denominator);
            }
            1 => prop_assert_eq!(next, parent_fee),
            _ => {
                prop_assert!(next >= parent_fee);
                prop_assert!(next - parent_fee <= (parent_fee / denominator).max(1));
            }
        }
    }
}
