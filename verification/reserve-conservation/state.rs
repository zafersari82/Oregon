//! Bounded state proof slice for Reserve Conservation Proof V1.
//! This file is compiled only by the pinned standalone Kani verifier.

#![forbid(unsafe_code)]

#[allow(dead_code)]
#[path = "src/model.rs"]
mod model;

use model::{
    apply_state_with_undo, undo_state, ModelEntry, ModelSlot, ModelState, ModelUndo, ProgramClass,
    StateError, StateSnapshot, StateTransition,
};

fn ordinary_slot(key: u8) -> ModelSlot {
    ModelSlot {
        key,
        entry: ModelEntry {
            amount: kani::any(),
            creation_height: kani::any(),
            is_coinbase: kani::any(),
            program: ProgramClass::Ordinary,
        },
    }
}

fn reserve_slot(key: u8, amount: u64, height: u64) -> ModelSlot {
    ModelSlot {
        key,
        entry: ModelEntry {
            amount,
            creation_height: height,
            is_coinbase: false,
            program: ProgramClass::Reserve,
        },
    }
}

fn symbolic_entry() -> ModelEntry {
    ModelEntry {
        amount: kani::any(),
        creation_height: kani::any(),
        is_coinbase: kani::any(),
        program: if kani::any::<bool>() {
            ProgramClass::Reserve
        } else {
            ProgramClass::Ordinary
        },
    }
}

fn symbolic_slot() -> ModelSlot {
    ModelSlot {
        key: kani::any(),
        entry: symbolic_entry(),
    }
}

fn symbolic_optional_slot() -> Option<ModelSlot> {
    if kani::any::<bool>() {
        Some(symbolic_slot())
    } else {
        None
    }
}

fn symbolic_state() -> ModelState {
    let state = ModelState {
        slots: [
            symbolic_optional_slot(),
            symbolic_optional_slot(),
            symbolic_optional_slot(),
            symbolic_optional_slot(),
        ],
    };
    assume_distinct_occupied_keys(&state);
    state
}

fn assume_distinct_occupied_keys(state: &ModelState) {
    for left in 0..4 {
        for right in (left + 1)..4 {
            if let (Some(a), Some(b)) = (state.slots[left], state.slots[right]) {
                kani::assume(a.key != b.key);
            }
        }
    }
}

fn occupied_count(state: &ModelState) -> usize {
    state.slots.iter().flatten().count()
}

fn reserve_count(state: &ModelState) -> usize {
    state
        .slots
        .iter()
        .flatten()
        .filter(|slot| slot.entry.program == ProgramClass::Reserve)
        .count()
}

fn contains_key(state: &ModelState, key: u8) -> bool {
    state.slots.iter().flatten().any(|slot| slot.key == key)
}

fn find_key(state: &ModelState, key: u8) -> Option<ModelSlot> {
    state
        .slots
        .iter()
        .flatten()
        .find(|slot| slot.key == key)
        .copied()
}

fn symbolic_transition() -> StateTransition {
    let has_previous: bool = kani::any();
    let has_new_key: bool = kani::any();
    StateTransition {
        previous: if has_previous {
            Some(StateSnapshot {
                key: kani::any(),
                amount: kani::any(),
            })
        } else {
            None
        },
        new_key: if has_new_key { Some(kani::any()) } else { None },
        new_amount: kani::any(),
        height: kani::any(),
    }
}

fn symbolic_undo() -> ModelUndo {
    ModelUndo {
        previous: if kani::any::<bool>() {
            Some(symbolic_slot())
        } else {
            None
        },
        created: if kani::any::<bool>() {
            Some(symbolic_slot())
        } else {
            None
        },
    }
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(5)]
fn rc03_successful_result_has_exact_reserve_cardinality() {
    let old_key: u8 = kani::any();
    let ordinary_key: u8 = kani::any();
    let new_key: u8 = kani::any();
    let has_previous: bool = kani::any();
    let old_amount: u64 = kani::any();
    let new_amount: u64 = kani::any();
    let height: u64 = kani::any();

    kani::assume(!has_previous || old_amount > 0);
    kani::assume(ordinary_key != old_key);
    kani::assume(new_key != ordinary_key && (!has_previous || new_key != old_key));

    let mut state = ModelState {
        slots: [
            Some(ordinary_slot(ordinary_key)),
            if has_previous {
                Some(reserve_slot(old_key, old_amount, kani::any()))
            } else {
                None
            },
            None,
            None,
        ],
    };
    let transition = StateTransition {
        previous: if has_previous {
            Some(StateSnapshot {
                key: old_key,
                amount: old_amount,
            })
        } else {
            None
        },
        new_key: if new_amount == 0 { None } else { Some(new_key) },
        new_amount,
        height,
    };

    let result = apply_state_with_undo(&mut state, transition);
    assert!(result.is_ok(), "RC03 valid bounded transition must apply");
    assert_eq!(
        reserve_count(&state),
        usize::from(new_amount > 0),
        "RC03 zero result has no reserve and positive result has exactly one"
    );

    kani::cover!(result.is_ok() && new_amount == 0, "RC03 zero-result success is reachable");
    kani::cover!(result.is_ok() && new_amount > 0, "RC03 positive-result success is reachable");
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(5)]
fn rc04_two_live_reserves_reject_unchanged() {
    let first_key: u8 = kani::any();
    let second_key: u8 = kani::any();
    let first_amount: u64 = kani::any();
    let second_amount: u64 = kani::any();
    kani::assume(first_key != second_key);

    let mut state = ModelState {
        slots: [
            Some(reserve_slot(first_key, first_amount, kani::any())),
            Some(reserve_slot(second_key, second_amount, kani::any())),
            None,
            None,
        ],
    };
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

    assert_eq!(result, Err(StateError::MultipleLiveReserves), "RC04 two reserves must reject");
    assert_eq!(state, before, "RC04 rejection must leave every slot unchanged");
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(5)]
fn rc05_successful_creation_has_exact_program_value_and_metadata() {
    let ordinary_key: u8 = kani::any();
    let new_key: u8 = kani::any();
    let new_amount: u64 = kani::any();
    let height: u64 = kani::any();
    kani::assume(new_amount > 0);
    kani::assume(new_key != ordinary_key);

    let ordinary = ordinary_slot(ordinary_key);
    let mut state = ModelState {
        slots: [Some(ordinary), None, None, None],
    };
    let result = apply_state_with_undo(
        &mut state,
        StateTransition {
            previous: None,
            new_key: Some(new_key),
            new_amount,
            height,
        },
    );

    assert!(result.is_ok(), "RC05 noncolliding bounded creation must apply");
    assert_eq!(find_key(&state, ordinary_key), Some(ordinary), "RC05 ordinary entry is preserved");
    assert_eq!(
        find_key(&state, new_key),
        Some(reserve_slot(new_key, new_amount, height)),
        "RC05 created reserve must have exact modeled program, value and metadata"
    );
    assert_eq!(reserve_count(&state), 1, "RC05 creation leaves exactly one reserve");
    kani::cover!(result.is_ok(), "RC05 successful creation is reachable");
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(5)]
fn rc06_failed_apply_and_failed_undo_are_atomic() {
    let mut apply_state = symbolic_state();
    let transition = symbolic_transition();
    if transition.previous.is_none() && transition.new_amount > 0 {
        kani::assume(occupied_count(&apply_state) <= 3);
    }
    let apply_before = apply_state;
    let apply_result = apply_state_with_undo(&mut apply_state, transition);
    if apply_result.is_err() {
        assert_eq!(apply_state, apply_before, "RC06 failed apply must leave every slot unchanged");
    }

    let mut undo_state_value = symbolic_state();
    let undo = symbolic_undo();
    let undo_before = undo_state_value;
    let undo_result = undo_state(&mut undo_state_value, &undo);
    if undo_result.is_err() {
        assert_eq!(undo_state_value, undo_before, "RC06 failed undo must leave every slot unchanged");
    }

    kani::cover!(apply_result.is_err(), "RC06 failed apply is reachable");
    kani::cover!(undo_result.is_err(), "RC06 failed undo is reachable");
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(5)]
fn rc07_apply_then_untampered_undo_restores_full_pre_state() {
    let old_key: u8 = kani::any();
    let ordinary_key: u8 = kani::any();
    let new_key: u8 = kani::any();
    let has_previous: bool = kani::any();
    let old_amount: u64 = kani::any();
    let new_amount: u64 = kani::any();
    kani::assume(!has_previous || old_amount > 0);
    kani::assume(ordinary_key != old_key);
    kani::assume(new_key != ordinary_key && (!has_previous || new_key != old_key));

    let mut state = ModelState {
        slots: [
            Some(ordinary_slot(ordinary_key)),
            if has_previous {
                Some(reserve_slot(old_key, old_amount, kani::any()))
            } else {
                None
            },
            None,
            None,
        ],
    };
    let before = state;
    let transition = StateTransition {
        previous: if has_previous {
            Some(StateSnapshot {
                key: old_key,
                amount: old_amount,
            })
        } else {
            None
        },
        new_key: if new_amount == 0 { None } else { Some(new_key) },
        new_amount,
        height: kani::any(),
    };

    let undo = apply_state_with_undo(&mut state, transition).expect("RC07 valid transition applies");
    let result = undo_state(&mut state, &undo);
    assert_eq!(result, Ok(()), "RC07 untampered undo must succeed");
    assert_eq!(state, before, "RC07 apply plus undo restores the whole pre-state");
    kani::cover!(result.is_ok(), "RC07 round trip is reachable");
}

#[kani::proof]
#[kani::solver(cadical)]
#[kani::unwind(5)]
fn rc10_occupied_output_and_tampered_undo_reject_unchanged() {
    let occupied_key: u8 = kani::any();
    let amount: u64 = kani::any();
    kani::assume(amount > 0);

    let ordinary = ordinary_slot(occupied_key);
    let mut collision_state = ModelState {
        slots: [Some(ordinary), None, None, None],
    };
    let collision_before = collision_state;
    let collision = apply_state_with_undo(
        &mut collision_state,
        StateTransition {
            previous: None,
            new_key: Some(occupied_key),
            new_amount: amount,
            height: kani::any(),
        },
    );
    assert_eq!(collision, Err(StateError::OutputCollision), "RC10 occupied output key must reject");
    assert_eq!(collision_state, collision_before, "RC10 collision rejection is atomic");

    let new_key: u8 = kani::any();
    kani::assume(new_key != occupied_key);
    let mut undo_state_value = ModelState {
        slots: [Some(ordinary), None, None, None],
    };
    let mut undo = apply_state_with_undo(
        &mut undo_state_value,
        StateTransition {
            previous: None,
            new_key: Some(new_key),
            new_amount: amount,
            height: kani::any(),
        },
    )
    .expect("RC10 setup creation applies");
    if let Some(created) = undo.created.as_mut() {
        created.entry.program = ProgramClass::Ordinary;
    }
    let undo_before = undo_state_value;
    let tampered = undo_state(&mut undo_state_value, &undo);
    assert_eq!(tampered, Err(StateError::UndoMismatch), "RC10 tampered undo must reject");
    assert_eq!(undo_state_value, undo_before, "RC10 tampered undo rejection is atomic");

    kani::cover!(collision == Err(StateError::OutputCollision), "RC10 collision rejection is reachable");
    kani::cover!(tampered == Err(StateError::UndoMismatch), "RC10 tampered undo rejection is reachable");
}
