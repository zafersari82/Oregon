//! First formal arithmetic slice for Reserve Conservation Proof V1.
//! This file is compiled only by the pinned standalone Kani verifier.

#![forbid(unsafe_code)]

#[path = "src/model.rs"]
mod model;

use model::{construct_arithmetic, ArithmeticError, ArithmeticInput, MAX_SUPPLY_BASE_UNITS};

fn symbolic_input() -> ArithmeticInput {
    let has_previous: bool = kani::any();
    let previous_amount: u64 = kani::any();
    ArithmeticInput {
        previous: if has_previous {
            Some(previous_amount)
        } else {
            None
        },
        native_deposit_total: kani::any(),
        execution_withdrawal_total: kani::any(),
        execution_fee_total: kani::any(),
        new_execution_balance_total: kani::any(),
    }
}

fn previous_as_i128(input: ArithmeticInput) -> i128 {
    i128::from(input.previous.unwrap_or(0))
}

fn mathematical_result(input: ArithmeticInput) -> i128 {
    previous_as_i128(input) + i128::from(input.native_deposit_total)
        - i128::from(input.execution_withdrawal_total)
        - i128::from(input.execution_fee_total)
}

fn independent_constructor_oracle(input: ArithmeticInput) -> Result<u64, ArithmeticError> {
    if input.previous == Some(0) {
        return Err(ArithmeticError::InvalidPreviousSnapshot);
    }

    let after_deposit = previous_as_i128(input) + i128::from(input.native_deposit_total);
    if after_deposit > i128::from(u64::MAX) {
        return Err(ArithmeticError::ArithmeticOverflow);
    }

    let after_withdrawal = after_deposit - i128::from(input.execution_withdrawal_total);
    if after_withdrawal < 0 {
        return Err(ArithmeticError::ArithmeticUnderflow);
    }

    let result = after_withdrawal - i128::from(input.execution_fee_total);
    if result < 0 {
        return Err(ArithmeticError::ArithmeticUnderflow);
    }

    let result = result as u64;
    if result != input.new_execution_balance_total {
        return Err(ArithmeticError::ExecutionBalanceMismatch);
    }

    if input
        .previous
        .is_some_and(|amount| amount > MAX_SUPPLY_BASE_UNITS)
        || result > MAX_SUPPLY_BASE_UNITS
    {
        return Err(ArithmeticError::AmountOutOfRange);
    }

    Ok(result)
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(1)]
fn rc01_accepted_arithmetic_matches_mathematical_equation() {
    let input = symbolic_input();
    let result = construct_arithmetic(input);

    if let Ok(accepted) = result {
        assert_eq!(
            i128::from(accepted),
            mathematical_result(input),
            "RC01 accepted arithmetic must equal the independent mathematical equation"
        );
    }

    kani::cover!(result.is_ok(), "RC01 accepted transition is reachable");
    kani::cover!(matches!(result, Ok(0)), "RC01 accepted zero result is reachable");
    kani::cover!(
        matches!(result, Ok(amount) if amount > 0),
        "RC01 accepted positive result is reachable"
    );
    kani::cover!(result.is_err(), "RC01 rejection is reachable");
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(1)]
fn rc02_accepted_result_equals_claimed_execution_total() {
    let input = symbolic_input();
    let result = construct_arithmetic(input);

    if let Ok(accepted) = result {
        assert_eq!(
            accepted, input.new_execution_balance_total,
            "RC02 accepted result must equal the caller-supplied execution total"
        );
    }

    kani::cover!(result.is_ok(), "RC02 accepted transition is reachable");
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(1)]
fn rc08_accepted_transition_conserves_reserve_arithmetic() {
    let input = symbolic_input();
    let result = construct_arithmetic(input);

    if let Ok(accepted) = result {
        assert_eq!(
            i128::from(accepted)
                + i128::from(input.execution_withdrawal_total)
                + i128::from(input.execution_fee_total),
            previous_as_i128(input) + i128::from(input.native_deposit_total),
            "RC08 accepted transition must conserve the bounded reserve arithmetic"
        );
    }

    kani::cover!(result.is_ok(), "RC08 accepted transition is reachable");
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(1)]
fn rc09_invalid_arithmetic_and_endpoints_reject() {
    let input = symbolic_input();
    let actual = construct_arithmetic(input);
    let expected = independent_constructor_oracle(input);

    assert_eq!(
        actual, expected,
        "RC09 constructor rejection class/order must match the independent i128 oracle"
    );

    kani::cover!(
        matches!(actual, Err(ArithmeticError::InvalidPreviousSnapshot)),
        "RC09 invalid present-zero snapshot is reachable"
    );
    kani::cover!(
        matches!(actual, Err(ArithmeticError::ArithmeticOverflow)),
        "RC09 checked-add overflow is reachable"
    );
    kani::cover!(
        matches!(actual, Err(ArithmeticError::ArithmeticUnderflow)),
        "RC09 checked-sub underflow is reachable"
    );
    kani::cover!(
        matches!(actual, Err(ArithmeticError::AmountOutOfRange)),
        "RC09 endpoint amount violation is reachable"
    );
}
