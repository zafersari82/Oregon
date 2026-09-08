//! Kani harnesses for the owner-approved bounded reserve-conservation model.
//!
//! This file is verification-only. It is not part of Oregon's production Cargo workspace.

#![forbid(unsafe_code)]

#[path = "src/model.rs"]
mod model;

use model::{
    apply_state_with_undo, construct_arithmetic, undo_state, ArithmeticError, ArithmeticInput,
    ModelEntry, ModelSlot, ModelState, ProgramClass, StateError, StateSnapshot, StateTransition,
    MAX_SUPPLY_BASE_UNITS,
};

macro_rules! symbolic_arithmetic_input {
    () => {{
        let has_previous: bool = kani::any();
        let previous_amount: u64 = kani::any();
        ArithmeticInput {
            previous: has_previous.then_some(previous_amount),
            native_deposit_total: kani::any(),
            execution_withdrawal_total: kani::any(),
            execution_fee_total: kani::any(),
            new_execution_balance_total: kani::any(),
        }
    }};
}

macro_rules! previous_as_i128 {
    ($input:expr) => {
        i128::from($input.previous.unwrap_or(0))
    };
}

macro_rules! reserve_slot {
    ($key:expr, $amount:expr, $height:expr) => {
        ModelSlot {
            key: $key,
            entry: ModelEntry {
                amount: $amount,
                creation_height: $height,
                is_coinbase: false,
                program: ProgramClass::Reserve,
            },
        }
    };
}

macro_rules! ordinary_slot {
    ($key:expr, $amount:expr, $height:expr, $coinbase:expr) => {
        ModelSlot {
            key: $key,
            entry: ModelEntry {
                amount: $amount,
                creation_height: $height,
                is_coinbase: $coinbase,
                program: ProgramClass::Ordinary,
            },
        }
    };
}

macro_rules! reserve_count {
    ($state:expr) => {
        $state
            .slots
            .iter()
            .flatten()
            .filter(|slot| slot.entry.program == ProgramClass::Reserve)
            .count()
    };
}

#[kani::proof]
#[kani::solver(cadical)]
fn rc01_arithmetic_matches_integer_equation() {
    let input = symbolic_arithmetic_input!();
    let result = construct_arithmetic(input);

    if let Ok(reserve) = result {
        let mathematical = previous_as_i128!(input) + i128::from(input.native_deposit_total)
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
    let input = symbolic_arithmetic_input!();
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
    let input = symbolic_arithmetic_input!();
    let result = construct_arithmetic(input);

    if let Ok(reserve) = result {
        let lhs = i128::from(reserve)
            + i128::from(input.execution_withdrawal_total)
            + i128::from(input.execution_fee_total);
        let rhs = previous_as_i128!(input) + i128::from(input.native_deposit_total);
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
    let input = symbolic_arithmetic_input!();
    let result = construct_arithmetic(input);
    let previous = previous_as_i128!(input);
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

#[kani::proof]
#[kani::solver(cadical)]
fn rc03_zero_and_positive_reserve_cardinality() {
    let positive: bool = kani::any();
    let has_previous: bool = kani::any();
    let previous_index_raw: u8 = kani::any();
    let ordinary_index_raw: u8 = kani::any();
    let previous_index = usize::from(previous_index_raw);
    let ordinary_index = usize::from(ordinary_index_raw);
    kani::assume(previous_index < 4);
    kani::assume(ordinary_index < 4);
    kani::assume(!has_previous || previous_index != ordinary_index);

    let previous_key: u8 = kani::any();
    let ordinary_key: u8 = kani::any();
    let new_key: u8 = kani::any();
    kani::assume(!has_previous || previous_key != ordinary_key);
    kani::assume(!positive || new_key != ordinary_key);
    kani::assume(!positive || !has_previous || new_key != previous_key);

    let previous_amount: u64 = kani::any();
    let new_amount: u64 = kani::any();
    kani::assume(!has_previous || (previous_amount > 0 && previous_amount <= MAX_SUPPLY_BASE_UNITS));
    kani::assume(!positive || (new_amount > 0 && new_amount <= MAX_SUPPLY_BASE_UNITS));

    let ordinary_amount: u64 = kani::any();
    let ordinary_height: u64 = kani::any();
    let ordinary_coinbase: bool = kani::any();
    let previous_height: u64 = kani::any();
    let transition_height: u64 = kani::any();

    let ordinary = ordinary_slot!(
        ordinary_key,
        ordinary_amount,
        ordinary_height,
        ordinary_coinbase
    );
    let mut state = ModelState {
        slots: [None, None, None, None],
    };
    state.slots[ordinary_index] = Some(ordinary);
    if has_previous {
        state.slots[previous_index] = Some(reserve_slot!(
            previous_key,
            previous_amount,
            previous_height
        ));
    }

    let transition = StateTransition {
        previous: has_previous.then_some(StateSnapshot {
            key: previous_key,
            amount: previous_amount,
        }),
        new_key: positive.then_some(new_key),
        new_amount: if positive { new_amount } else { 0 },
        height: transition_height,
    };
    let result = apply_state_with_undo(&mut state, transition);

    assert!(result.is_ok(), "RC03 constructed valid state transition is accepted");
    assert!(
        reserve_count!(state) == usize::from(positive),
        "RC03 accepted zero result has no reserve and positive result has exactly one"
    );
    assert!(
        state.slots.contains(&Some(ordinary)),
        "RC03 unrelated ordinary entry remains unchanged"
    );

    kani::cover!(
        result.is_ok() && !positive && reserve_count!(state) == 0,
        "RC03 accepted zero-result branch is reachable"
    );
    kani::cover!(
        result.is_ok() && positive && reserve_count!(state) == 1,
        "RC03 accepted positive-result branch is reachable"
    );
}

#[kani::proof]
#[kani::solver(cadical)]
fn rc04_multiple_live_reserves_reject_unchanged() {
    let first_index_raw: u8 = kani::any();
    let second_index_raw: u8 = kani::any();
    let ordinary_index_raw: u8 = kani::any();
    let first_index = usize::from(first_index_raw);
    let second_index = usize::from(second_index_raw);
    let ordinary_index = usize::from(ordinary_index_raw);
    kani::assume(first_index < 4);
    kani::assume(second_index < 4);
    kani::assume(ordinary_index < 4);
    kani::assume(first_index != second_index);
    kani::assume(first_index != ordinary_index);
    kani::assume(second_index != ordinary_index);

    let first_key: u8 = kani::any();
    let second_key: u8 = kani::any();
    let ordinary_key: u8 = kani::any();
    kani::assume(first_key != second_key);
    kani::assume(first_key != ordinary_key);
    kani::assume(second_key != ordinary_key);

    let first_amount: u64 = kani::any();
    let second_amount: u64 = kani::any();
    let ordinary_amount: u64 = kani::any();
    let first_height: u64 = kani::any();
    let second_height: u64 = kani::any();
    let ordinary_height: u64 = kani::any();
    let ordinary_coinbase: bool = kani::any();

    let mut state = ModelState {
        slots: [None, None, None, None],
    };
    state.slots[first_index] = Some(reserve_slot!(first_key, first_amount, first_height));
    state.slots[second_index] = Some(reserve_slot!(second_key, second_amount, second_height));
    state.slots[ordinary_index] = Some(ordinary_slot!(
        ordinary_key,
        ordinary_amount,
        ordinary_height,
        ordinary_coinbase
    ));
    let before = state;

    let result = apply_state_with_undo(
        &mut state,
        StateTransition {
            previous: Some(StateSnapshot {
                key: first_key,
                amount: first_amount,
            }),
            new_key: None,
            new_amount: 0,
            height: kani::any(),
        },
    );

    assert!(
        result == Err(StateError::MultipleLiveReserves),
        "RC04 two live reserves reject with the multiple-reserve error"
    );
    assert!(state == before, "RC04 two-live-reserve rejection leaves the full state unchanged");
    kani::cover!(
        result == Err(StateError::MultipleLiveReserves) && state == before,
        "RC04 multiple-live-reserve rejection is reachable"
    );
}

#[kani::proof]
#[kani::solver(cadical)]
fn rc05_created_reserve_exact_entry() {
    let ordinary_index_raw: u8 = kani::any();
    let ordinary_index = usize::from(ordinary_index_raw);
    kani::assume(ordinary_index < 4);

    let ordinary_key: u8 = kani::any();
    let new_key: u8 = kani::any();
    kani::assume(new_key != ordinary_key);

    let new_amount: u64 = kani::any();
    kani::assume(new_amount > 0 && new_amount <= MAX_SUPPLY_BASE_UNITS);
    let height: u64 = kani::any();
    let ordinary = ordinary_slot!(
        ordinary_key,
        kani::any(),
        kani::any(),
        kani::any()
    );
    let mut state = ModelState {
        slots: [None, None, None, None],
    };
    state.slots[ordinary_index] = Some(ordinary);

    let result = apply_state_with_undo(
        &mut state,
        StateTransition {
            previous: None,
            new_key: Some(new_key),
            new_amount,
            height,
        },
    );
    let expected = reserve_slot!(new_key, new_amount, height);

    assert!(result.is_ok(), "RC05 valid reserve creation is accepted");
    if let Ok(undo) = result {
        assert!(
            undo.created == Some(expected),
            "RC05 undo records the exact created reserve entry"
        );
        assert!(
            state.slots.contains(&Some(expected)),
            "RC05 successful creation has exact reserve program value and metadata"
        );
        assert!(
            state.slots.contains(&Some(ordinary)),
            "RC05 unrelated ordinary entry remains byte-for-byte modeled-equal"
        );
    }

    kani::cover!(
        result.is_ok() && state.slots.contains(&Some(expected)),
        "RC05 exact reserve creation is reachable"
    );
}

#[kani::proof]
#[kani::solver(cadical)]
fn rc06_failed_apply_and_undo_are_atomic() {
    let reserve_index_raw: u8 = kani::any();
    let ordinary_index_raw: u8 = kani::any();
    let reserve_index = usize::from(reserve_index_raw);
    let ordinary_index = usize::from(ordinary_index_raw);
    kani::assume(reserve_index < 4);
    kani::assume(ordinary_index < 4);
    kani::assume(reserve_index != ordinary_index);

    let reserve_key: u8 = kani::any();
    let collision_key: u8 = kani::any();
    kani::assume(reserve_key != collision_key);
    let reserve_amount: u64 = kani::any();
    kani::assume(reserve_amount > 0 && reserve_amount <= MAX_SUPPLY_BASE_UNITS);
    let new_amount: u64 = kani::any();
    kani::assume(new_amount > 0 && new_amount <= MAX_SUPPLY_BASE_UNITS);

    let mut apply_state = ModelState {
        slots: [None, None, None, None],
    };
    apply_state.slots[reserve_index] = Some(reserve_slot!(reserve_key, reserve_amount, kani::any()));
    apply_state.slots[ordinary_index] = Some(ordinary_slot!(
        collision_key,
        kani::any(),
        kani::any(),
        kani::any()
    ));
    let apply_before = apply_state;
    let apply_result = apply_state_with_undo(
        &mut apply_state,
        StateTransition {
            previous: Some(StateSnapshot {
                key: reserve_key,
                amount: reserve_amount,
            }),
            new_key: Some(collision_key),
            new_amount,
            height: kani::any(),
        },
    );
    assert!(
        apply_result == Err(StateError::OutputCollision),
        "RC06 occupied output collision rejects apply"
    );
    assert!(
        apply_state == apply_before,
        "RC06 failed apply leaves every modeled entry unchanged"
    );

    let ordinary_key: u8 = kani::any();
    let created_key: u8 = kani::any();
    kani::assume(ordinary_key != created_key);
    let created_amount: u64 = kani::any();
    kani::assume(created_amount > 0 && created_amount <= MAX_SUPPLY_BASE_UNITS);
    let created_height: u64 = kani::any();
    let ordinary = ordinary_slot!(ordinary_key, kani::any(), kani::any(), kani::any());
    let mut undo_state_value = ModelState {
        slots: [Some(ordinary), None, None, None],
    };
    let setup = apply_state_with_undo(
        &mut undo_state_value,
        StateTransition {
            previous: None,
            new_key: Some(created_key),
            new_amount: created_amount,
            height: created_height,
        },
    );
    assert!(setup.is_ok(), "RC06 undo-atomicity setup apply is reachable");
    if let Ok(undo) = setup {
        let created_index = undo_state_value
            .slots
            .iter()
            .position(|slot| slot.is_some_and(|slot| slot.key == created_key));
        assert!(created_index.is_some(), "RC06 setup created reserve is present");
        if let Some(index) = created_index {
            undo_state_value.slots[index]
                .as_mut()
                .unwrap()
                .entry
                .creation_height = created_height.wrapping_add(1);
            let undo_before = undo_state_value;
            let undo_result = undo_state(&mut undo_state_value, &undo);
            assert!(
                undo_result == Err(StateError::UndoMismatch),
                "RC06 tampered current reserve rejects undo"
            );
            assert!(
                undo_state_value == undo_before,
                "RC06 failed undo leaves every modeled entry unchanged"
            );
            kani::cover!(
                undo_result == Err(StateError::UndoMismatch) && undo_state_value == undo_before,
                "RC06 failed-undo atomicity branch is reachable"
            );
        }
    }

    kani::cover!(
        apply_result == Err(StateError::OutputCollision) && apply_state == apply_before,
        "RC06 failed-apply atomicity branch is reachable"
    );
}

#[kani::proof]
#[kani::solver(cadical)]
fn rc07_apply_then_undo_restores_full_state() {
    let has_previous: bool = kani::any();
    let positive: bool = kani::any();
    let previous_index_raw: u8 = kani::any();
    let ordinary_index_raw: u8 = kani::any();
    let previous_index = usize::from(previous_index_raw);
    let ordinary_index = usize::from(ordinary_index_raw);
    kani::assume(previous_index < 4);
    kani::assume(ordinary_index < 4);
    kani::assume(!has_previous || previous_index != ordinary_index);

    let previous_key: u8 = kani::any();
    let ordinary_key: u8 = kani::any();
    let created_key: u8 = kani::any();
    kani::assume(!has_previous || previous_key != ordinary_key);
    kani::assume(!positive || created_key != ordinary_key);
    kani::assume(!positive || !has_previous || created_key != previous_key);

    let previous_amount: u64 = kani::any();
    let created_amount: u64 = kani::any();
    kani::assume(!has_previous || (previous_amount > 0 && previous_amount <= MAX_SUPPLY_BASE_UNITS));
    kani::assume(!positive || (created_amount > 0 && created_amount <= MAX_SUPPLY_BASE_UNITS));

    let ordinary = ordinary_slot!(ordinary_key, kani::any(), kani::any(), kani::any());
    let mut state = ModelState {
        slots: [None, None, None, None],
    };
    state.slots[ordinary_index] = Some(ordinary);
    if has_previous {
        state.slots[previous_index] = Some(reserve_slot!(previous_key, previous_amount, kani::any()));
    }
    let before = state;

    let apply_result = apply_state_with_undo(
        &mut state,
        StateTransition {
            previous: has_previous.then_some(StateSnapshot {
                key: previous_key,
                amount: previous_amount,
            }),
            new_key: positive.then_some(created_key),
            new_amount: if positive { created_amount } else { 0 },
            height: kani::any(),
        },
    );
    assert!(apply_result.is_ok(), "RC07 constructed valid apply is accepted");
    if let Ok(undo) = apply_result {
        let undo_result = undo_state(&mut state, &undo);
        assert!(undo_result.is_ok(), "RC07 untampered undo is accepted");
        assert!(
            state == before,
            "RC07 apply followed by untampered undo restores the complete pre-state"
        );
        kani::cover!(
            !has_previous && !positive && undo_result.is_ok() && state == before,
            "RC07 empty-to-empty round trip is reachable"
        );
        kani::cover!(
            !has_previous && positive && undo_result.is_ok() && state == before,
            "RC07 creation round trip is reachable"
        );
        kani::cover!(
            has_previous && !positive && undo_result.is_ok() && state == before,
            "RC07 removal-to-zero round trip is reachable"
        );
        kani::cover!(
            has_previous && positive && undo_result.is_ok() && state == before,
            "RC07 replacement round trip is reachable"
        );
    }
}

#[kani::proof]
#[kani::solver(cadical)]
fn rc10_collisions_and_tampered_undo_reject_unchanged() {
    let occupied_key: u8 = kani::any();
    let collision_amount: u64 = kani::any();
    kani::assume(collision_amount > 0 && collision_amount <= MAX_SUPPLY_BASE_UNITS);
    let occupied = ordinary_slot!(occupied_key, kani::any(), kani::any(), kani::any());
    let mut collision_state = ModelState {
        slots: [Some(occupied), None, None, None],
    };
    let collision_before = collision_state;
    let collision_result = apply_state_with_undo(
        &mut collision_state,
        StateTransition {
            previous: None,
            new_key: Some(occupied_key),
            new_amount: collision_amount,
            height: kani::any(),
        },
    );
    assert!(
        collision_result == Err(StateError::OutputCollision),
        "RC10 occupied output key rejects apply"
    );
    assert!(
        collision_state == collision_before,
        "RC10 occupied-output rejection leaves the full state unchanged"
    );

    let ordinary_key: u8 = kani::any();
    let created_key: u8 = kani::any();
    kani::assume(ordinary_key != created_key);
    let created_amount: u64 = kani::any();
    kani::assume(created_amount > 0 && created_amount <= MAX_SUPPLY_BASE_UNITS);
    let created_height: u64 = kani::any();
    let ordinary = ordinary_slot!(ordinary_key, kani::any(), kani::any(), kani::any());
    let mut state = ModelState {
        slots: [Some(ordinary), None, None, None],
    };
    let setup = apply_state_with_undo(
        &mut state,
        StateTransition {
            previous: None,
            new_key: Some(created_key),
            new_amount: created_amount,
            height: created_height,
        },
    );
    assert!(setup.is_ok(), "RC10 stale-undo setup apply is accepted");
    if let Ok(mut stale_undo) = setup {
        if let Some(created) = stale_undo.created.as_mut() {
            created.entry.creation_height = created.entry.creation_height.wrapping_add(1);
        }
        let before_undo = state;
        let undo_result = undo_state(&mut state, &stale_undo);
        assert!(
            undo_result == Err(StateError::UndoMismatch),
            "RC10 stale or tampered created-entry undo rejects"
        );
        assert!(
            state == before_undo,
            "RC10 stale or tampered undo rejection leaves the full state unchanged"
        );
        kani::cover!(
            undo_result == Err(StateError::UndoMismatch) && state == before_undo,
            "RC10 stale-undo rejection is reachable"
        );
    }

    kani::cover!(
        collision_result == Err(StateError::OutputCollision) && collision_state == collision_before,
        "RC10 occupied-output collision rejection is reachable"
    );
}
