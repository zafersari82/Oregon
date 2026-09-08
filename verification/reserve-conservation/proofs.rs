//! Kani harnesses for the owner-approved bounded reserve-conservation model.
//!
//! This file is verification-only. It is not part of Oregon's production Cargo workspace.

#![forbid(unsafe_code)]

#[path = "src/model.rs"]
mod model;

use model::{construct_arithmetic, ArithmeticError, ArithmeticInput, MAX_SUPPLY_BASE_UNITS};

fn symbolic_arithmetic_input() -> ArithmeticInput {
    let has_previous: bool = kani::any();
    let previous_amount: u64 = kani::any();
    ArithmeticInput {
        previous: has_previous.then_some(previous_amount),
        native_deposit_total: kani::any(),
        execution_withdrawal_total: kani::any(),
        execution_fee_total: kani::any(),
        new_execution_balance_total: kani::any(),
    }
}

fn previous_as_i128(input: ArithmeticInput) -> i128 {
    i128::from(input.previous.unwrap_or(0))
}

#[kani::proof]
#[kani::solver(cadical)]
fn rc01_arithmetic_matches_integer_equation() {
    let input = symbolic_arithmetic_input();
    let result = construct_arithmetic(input);

    if let Ok(reserve) = result {
        let mathematical = previous_as_i128(input) + i128::from(input.native_deposit_total)
            - i128::from(input.execution_withdrawal_total)
            - i128::from(input.execution_fee_total);
        assert!(
            i128::from(reserve) == mathematical,
            "RC01 accepted arithmetic matches the independent integer equation"
        );
    }

    kani::cover!(result.is_ok(), "RC01 accepted transition is reachable");
    kani::cover!(
        result == Err(ArithmeticError::ArithmeticOverflow),
        "RC01 checked-add overflow rejection is reachable"
    );
}

#[kani::proof]
#[kani::solver(cadical)]
fn rc02_result_matches_claimed_execution_total() {
    let input = symbolic_arithmetic_input();
    let result = construct_arithmetic(input);

    if let Ok(reserve) = result {
        assert!(
            reserve == input.new_execution_balance_total,
            "RC02 accepted reserve equals the claimed execution total"
        );
    }

    kani::cover!(result.is_ok(), "RC02 accepted transition is reachable");
    kani::cover!(
        result == Err(ArithmeticError::ExecutionBalanceMismatch),
        "RC02 execution-balance mismatch rejection is reachable"
    );
}

#[kani::proof]
#[kani::solver(cadical)]
fn rc08_conservation_identity() {
    let input = symbolic_arithmetic_input();
    let result = construct_arithmetic(input);

    if let Ok(reserve) = result {
        let lhs = i128::from(reserve)
            + i128::from(input.execution_withdrawal_total)
            + i128::from(input.execution_fee_total);
        let rhs = previous_as_i128(input) + i128::from(input.native_deposit_total);
        assert!(
            lhs == rhs,
            "RC08 accepted transition conserves reserve arithmetic as mathematical integers"
        );
    }

    kani::cover!(result.is_ok(), "RC08 accepted transition is reachable");
}

#[kani::proof]
#[kani::solver(cadical)]
fn rc09_invalid_arithmetic_and_endpoints_reject() {
    let input = symbolic_arithmetic_input();
    let result = construct_arithmetic(input);
    let previous = previous_as_i128(input);
    let after_deposit = previous + i128::from(input.native_deposit_total);
    let after_withdrawal = after_deposit - i128::from(input.execution_withdrawal_total);
    let mathematical_result = after_withdrawal - i128::from(input.execution_fee_total);
    let u64_max = i128::from(u64::MAX);

    if input.previous == Some(0) {
        assert!(
            result == Err(ArithmeticError::InvalidPreviousSnapshot),
            "RC09 present zero previous snapshot rejects"
        );
    }

    if input.previous != Some(0) && after_deposit <= u64_max && after_withdrawal < 0 {
        assert!(
            result == Err(ArithmeticError::ArithmeticUnderflow),
            "RC09 withdrawal underflow rejects"
        );
    }

    if input.previous != Some(0)
        && after_deposit <= u64_max
        && after_withdrawal >= 0
        && mathematical_result < 0
    {
        assert!(
            result == Err(ArithmeticError::ArithmeticUnderflow),
            "RC09 fee underflow rejects"
        );
    }

    let arithmetic_is_u64 = input.previous != Some(0)
        && after_deposit <= u64_max
        && after_withdrawal >= 0
        && mathematical_result >= 0
        && mathematical_result <= u64_max;
    let claimed_matches = mathematical_result == i128::from(input.new_execution_balance_total);

    if arithmetic_is_u64
        && claimed_matches
        && input
            .previous
            .is_some_and(|amount| amount > MAX_SUPPLY_BASE_UNITS)
    {
        assert!(
            result == Err(ArithmeticError::AmountOutOfRange),
            "RC09 previous endpoint amount violation rejects"
        );
    }

    if arithmetic_is_u64
        && claimed_matches
        && input
            .previous
            .is_none_or(|amount| amount <= MAX_SUPPLY_BASE_UNITS)
        && mathematical_result > i128::from(MAX_SUPPLY_BASE_UNITS)
    {
        assert!(
            result == Err(ArithmeticError::AmountOutOfRange),
            "RC09 resulting endpoint amount violation rejects"
        );
    }

    kani::cover!(
        result == Err(ArithmeticError::InvalidPreviousSnapshot),
        "RC09 invalid-previous rejection is reachable"
    );
    kani::cover!(
        result == Err(ArithmeticError::ArithmeticUnderflow),
        "RC09 underflow rejection is reachable"
    );
    kani::cover!(
        result == Err(ArithmeticError::AmountOutOfRange),
        "RC09 endpoint-range rejection is reachable"
    );
}
