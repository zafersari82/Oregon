//! Deliberately broken arithmetic variants used to prove that the RC obligations are sensitive.
//!
//! These harnesses are verification-only negative controls. They never enter Oregon's
//! production workspace or the positive reserve model.

#![forbid(unsafe_code)]

#[path = "src/model.rs"]
mod model;

use model::MAX_SUPPLY_BASE_UNITS;

#[kani::proof]
#[kani::solver(cadical)]
fn control_rc01_wrapping_add() {
    let previous: u64 = kani::any();
    let deposit: u64 = kani::any();
    kani::assume(previous.checked_add(deposit).is_none());

    // Intentional mutation: checked addition is replaced with wrapping addition.
    let mutated = previous.wrapping_add(deposit);
    assert!(
        i128::from(mutated) == i128::from(previous) + i128::from(deposit),
        "RC01 control: wrapping addition must not satisfy the integer equation on overflow"
    );
}

#[kani::proof]
#[kani::solver(cadical)]
fn control_rc02_missing_execution_equality() {
    let previous: u64 = kani::any();
    let deposit: u64 = kani::any();
    let withdrawal: u64 = kani::any();
    let fee: u64 = kani::any();
    let claimed_execution_total: u64 = kani::any();

    kani::assume(previous <= MAX_SUPPLY_BASE_UNITS);
    let Some(after_deposit) = previous.checked_add(deposit) else {
        return;
    };
    let Some(after_withdrawal) = after_deposit.checked_sub(withdrawal) else {
        return;
    };
    let Some(reserve) = after_withdrawal.checked_sub(fee) else {
        return;
    };
    kani::assume(reserve <= MAX_SUPPLY_BASE_UNITS);
    kani::assume(reserve != claimed_execution_total);

    // Intentional mutation: the equality rejection against E is removed.
    let mutated_accepted_reserve = reserve;
    assert!(
        mutated_accepted_reserve == claimed_execution_total,
        "RC02 control: removing the execution-total equality check must admit a mismatch"
    );
}

#[kani::proof]
#[kani::solver(cadical)]
fn control_rc08_omit_fee_subtraction() {
    let previous: u64 = kani::any();
    let deposit: u64 = kani::any();
    let withdrawal: u64 = kani::any();
    let fee: u64 = kani::any();

    let Some(after_deposit) = previous.checked_add(deposit) else {
        return;
    };
    let Some(after_withdrawal) = after_deposit.checked_sub(withdrawal) else {
        return;
    };
    kani::assume(fee > 0);
    kani::assume(after_withdrawal >= fee);

    // Intentional mutation: fee subtraction is omitted from the resulting reserve.
    let mutated_reserve = after_withdrawal;
    let lhs = i128::from(mutated_reserve) + i128::from(withdrawal) + i128::from(fee);
    let rhs = i128::from(previous) + i128::from(deposit);
    assert!(
        lhs == rhs,
        "RC08 control: omitting fee subtraction must violate conservation"
    );
}

#[kani::proof]
#[kani::solver(cadical)]
fn control_rc09_saturating_subtraction() {
    let previous: u64 = kani::any();
    let deposit: u64 = kani::any();
    let withdrawal: u64 = kani::any();

    let Some(after_deposit) = previous.checked_add(deposit) else {
        return;
    };
    kani::assume(withdrawal > after_deposit);

    // Intentional mutation: an underflowing withdrawal is silently saturated and accepted.
    let mutated_accepted = Some(after_deposit.saturating_sub(withdrawal));
    assert!(
        mutated_accepted.is_none(),
        "RC09 control: saturating subtraction must not accept an underflowing transition"
    );
}
