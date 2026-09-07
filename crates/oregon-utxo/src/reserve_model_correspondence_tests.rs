#[path = "../../../verification/reserve-conservation/src/model.rs"]
mod reserve_model;

use oregon_primitives::{Hash256, OutPoint};

use crate::{
    ReservePoolSnapshotV1, ReserveTransitionError, ReserveTransitionV1, ReserveTransitionV1Parts,
};
use reserve_model::{ArithmeticError, ArithmeticInput, MAX_SUPPLY_BASE_UNITS};

fn outpoint() -> OutPoint {
    OutPoint {
        txid: Hash256::from_bytes([0x90; 32]),
        index: 0,
    }
}

fn production_result(input: ArithmeticInput) -> Result<u64, &'static str> {
    ReserveTransitionV1::new(ReserveTransitionV1Parts {
        chain_id: 7,
        height: 100,
        parent_block_hash: Hash256::from_bytes([0x10; 32]),
        previous: input.previous.map(|amount| ReservePoolSnapshotV1 {
            outpoint: outpoint(),
            amount,
        }),
        native_deposit_total: input.native_deposit_total,
        execution_withdrawal_total: input.execution_withdrawal_total,
        execution_fee_total: input.execution_fee_total,
        new_execution_balance_total: input.new_execution_balance_total,
        producer_coinbase_txid: Hash256::from_bytes([0x20; 32]),
    })
    .map(|transition| transition.new_reserve_amount())
    .map_err(|error| match error {
        ReserveTransitionError::ArithmeticOverflow => "arithmetic_overflow",
        ReserveTransitionError::ArithmeticUnderflow => "arithmetic_underflow",
        ReserveTransitionError::ExecutionBalanceMismatch { .. } => "execution_balance_mismatch",
        ReserveTransitionError::InvalidPreviousSnapshot => "invalid_previous_snapshot",
        ReserveTransitionError::AmountOutOfRange => "amount_out_of_range",
        _ => "state_transition_error",
    })
}

fn model_result(input: ArithmeticInput) -> Result<u64, &'static str> {
    reserve_model::construct_arithmetic(input).map_err(|error| match error {
        ArithmeticError::ArithmeticOverflow => "arithmetic_overflow",
        ArithmeticError::ArithmeticUnderflow => "arithmetic_underflow",
        ArithmeticError::ExecutionBalanceMismatch => "execution_balance_mismatch",
        ArithmeticError::InvalidPreviousSnapshot => "invalid_previous_snapshot",
        ArithmeticError::AmountOutOfRange => "amount_out_of_range",
    })
}

#[test]
fn correspondence_pins_present_zero_snapshot_error_precedence() {
    let input = ArithmeticInput {
        previous: Some(0),
        native_deposit_total: 1,
        execution_withdrawal_total: 0,
        execution_fee_total: 0,
        new_execution_balance_total: 1,
    };

    assert_eq!(model_result(input), production_result(input));
}

#[test]
fn correspondence_pins_intermediate_overflow_before_endpoint_range_check() {
    let input = ArithmeticInput {
        previous: Some(u64::MAX),
        native_deposit_total: 1,
        execution_withdrawal_total: u64::MAX,
        execution_fee_total: 0,
        new_execution_balance_total: 1,
    };

    assert_eq!(
        production_result(input),
        Err("arithmetic_overflow"),
        "production must reject the overflowing intermediate even though later subtraction could be small"
    );
    assert_eq!(model_result(input), production_result(input));
}

#[test]
fn correspondence_pins_previous_endpoint_supply_bound_after_arithmetic() {
    let input = ArithmeticInput {
        previous: Some(MAX_SUPPLY_BASE_UNITS + 1),
        native_deposit_total: 0,
        execution_withdrawal_total: 1,
        execution_fee_total: 0,
        new_execution_balance_total: MAX_SUPPLY_BASE_UNITS,
    };

    assert_eq!(production_result(input), Err("amount_out_of_range"));
    assert_eq!(model_result(input), production_result(input));
}

#[test]
fn correspondence_does_not_invent_supply_caps_for_flow_amounts() {
    let input = ArithmeticInput {
        previous: None,
        native_deposit_total: MAX_SUPPLY_BASE_UNITS + 1,
        execution_withdrawal_total: 1,
        execution_fee_total: 0,
        new_execution_balance_total: MAX_SUPPLY_BASE_UNITS,
    };

    assert_eq!(production_result(input), Ok(MAX_SUPPLY_BASE_UNITS));
    assert_eq!(model_result(input), production_result(input));
}
