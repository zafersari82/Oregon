#[path = "../../../verification/reserve-conservation/src/model.rs"]
mod reserve_model;

use oregon_primitives::execution_reserve::EXECUTION_RESERVE_LOCKING_PROGRAM_V1;
use oregon_primitives::{Amount, Hash256, OutPoint, TxOutput};

use crate::{
    ReservePoolSnapshotV1, ReserveTransitionError, ReserveTransitionV1, ReserveTransitionV1Parts,
    UtxoEntry, UtxoState,
};
use reserve_model::{
    ArithmeticError, ArithmeticInput, MAX_SUPPLY_BASE_UNITS, ModelEntry, ModelSlot, ModelState,
    ProgramClass, StateError, StateSnapshot, StateTransition,
};

fn outpoint() -> OutPoint {
    tagged_outpoint(0x90)
}

fn tagged_outpoint(tag: u8) -> OutPoint {
    OutPoint {
        txid: Hash256::from_bytes([tag; 32]),
        index: 0,
    }
}

fn reserve_entry(amount: u64) -> UtxoEntry {
    UtxoEntry {
        output: TxOutput {
            value: Amount::from_base_units(amount).unwrap(),
            locking_program: EXECUTION_RESERVE_LOCKING_PROGRAM_V1.to_vec(),
        },
        creation_height: 99,
        is_coinbase: false,
    }
}

fn ordinary_entry(amount: u64) -> UtxoEntry {
    UtxoEntry {
        output: TxOutput {
            value: Amount::from_base_units(amount).unwrap(),
            locking_program: vec![0x01, 0x02, 0x03],
        },
        creation_height: 88,
        is_coinbase: true,
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

#[test]
fn correspondence_pins_multiple_live_reserve_rejection_and_atomicity() {
    let first = tagged_outpoint(0x91);
    let second = tagged_outpoint(0x92);
    let ordinary = tagged_outpoint(0x93);
    let transition = ReserveTransitionV1::new(ReserveTransitionV1Parts {
        chain_id: 7,
        height: 100,
        parent_block_hash: Hash256::from_bytes([0x10; 32]),
        previous: Some(ReservePoolSnapshotV1 {
            outpoint: first,
            amount: 100,
        }),
        native_deposit_total: 0,
        execution_withdrawal_total: 100,
        execution_fee_total: 0,
        new_execution_balance_total: 0,
        producer_coinbase_txid: Hash256::from_bytes([0x20; 32]),
    })
    .unwrap();

    let mut production_state = UtxoState::try_from_entries([
        (first, reserve_entry(100)),
        (second, reserve_entry(50)),
        (ordinary, ordinary_entry(7)),
    ])
    .unwrap();
    let production_before = production_state.clone();
    let production_apply =
        crate::reserve::apply_reserve_transition_v1(&mut production_state, &transition)
            .map(|_| ())
            .map_err(|error| match error {
                ReserveTransitionError::MultipleLiveReserves => "multiple_live_reserves",
                ReserveTransitionError::PreviousReserveMismatch => "previous_reserve_mismatch",
                ReserveTransitionError::OutputCollision(_) => "output_collision",
                _ => "other_state_error",
            });

    let reserve = |key, amount| ModelSlot {
        key,
        entry: ModelEntry {
            amount,
            creation_height: 99,
            is_coinbase: false,
            program: ProgramClass::Reserve,
        },
    };
    let ordinary = ModelSlot {
        key: 3,
        entry: ModelEntry {
            amount: 7,
            creation_height: 88,
            is_coinbase: true,
            program: ProgramClass::Ordinary,
        },
    };
    let mut model_state = ModelState {
        slots: [
            Some(reserve(1, 100)),
            Some(reserve(2, 50)),
            Some(ordinary),
            None,
        ],
    };
    let model_before = model_state;
    let model_apply = reserve_model::apply_state(
        &mut model_state,
        StateTransition {
            previous: Some(StateSnapshot {
                key: 1,
                amount: 100,
            }),
            new_key: None,
            new_amount: 0,
            height: 100,
        },
    )
    .map_err(|error| match error {
        StateError::MultipleLiveReserves => "multiple_live_reserves",
        StateError::PreviousReserveMismatch => "previous_reserve_mismatch",
        StateError::OutputCollision => "output_collision",
        StateError::UndoMismatch => "undo_mismatch",
    });

    assert_eq!(production_apply, Err("multiple_live_reserves"));
    assert_eq!(production_state, production_before);
    assert_eq!(model_apply, production_apply);
    assert_eq!(model_state, model_before);
}

#[test]
fn correspondence_pins_tampered_undo_rejection_and_atomicity() {
    let ordinary = tagged_outpoint(0xa1);
    let transition = ReserveTransitionV1::new(ReserveTransitionV1Parts {
        chain_id: 7,
        height: 100,
        parent_block_hash: Hash256::from_bytes([0x10; 32]),
        previous: None,
        native_deposit_total: 10,
        execution_withdrawal_total: 0,
        execution_fee_total: 0,
        new_execution_balance_total: 10,
        producer_coinbase_txid: Hash256::from_bytes([0x20; 32]),
    })
    .unwrap();

    let mut production_state =
        UtxoState::try_from_entries([(ordinary, ordinary_entry(7))]).unwrap();
    let production_undo =
        crate::reserve::apply_reserve_transition_v1(&mut production_state, &transition).unwrap();
    let created = transition.new_reserve_outpoint().unwrap();
    let mut tampered_entries: Vec<_> = production_state
        .entries()
        .map(|(outpoint, entry)| (*outpoint, entry.clone()))
        .collect();
    let created_entry = tampered_entries
        .iter_mut()
        .find(|(outpoint, _)| *outpoint == created)
        .map(|(_, entry)| entry)
        .unwrap();
    created_entry.creation_height = 101;
    production_state = UtxoState::try_from_entries(tampered_entries).unwrap();
    let production_before = production_state.clone();
    let production_undo_result =
        crate::reserve::undo_reserve_transition_v1(&mut production_state, &production_undo)
            .map_err(|error| match error {
                ReserveTransitionError::UndoMismatch => "undo_mismatch",
                _ => "other_state_error",
            });

    let ordinary = ModelSlot {
        key: 3,
        entry: ModelEntry {
            amount: 7,
            creation_height: 88,
            is_coinbase: true,
            program: ProgramClass::Ordinary,
        },
    };
    let mut model_state = ModelState {
        slots: [Some(ordinary), None, None, None],
    };
    let model_undo = reserve_model::apply_state_with_undo(
        &mut model_state,
        StateTransition {
            previous: None,
            new_key: Some(7),
            new_amount: 10,
            height: 100,
        },
    )
    .unwrap();
    let created_index = model_state
        .slots
        .iter()
        .position(|slot| slot.is_some_and(|slot| slot.key == 7))
        .unwrap();
    model_state.slots[created_index]
        .as_mut()
        .unwrap()
        .entry
        .creation_height = 101;
    let model_before = model_state;
    let model_undo_result =
        reserve_model::undo_state(&mut model_state, &model_undo).map_err(|error| match error {
            StateError::UndoMismatch => "undo_mismatch",
            StateError::MultipleLiveReserves => "multiple_live_reserves",
            StateError::PreviousReserveMismatch => "previous_reserve_mismatch",
            StateError::OutputCollision => "output_collision",
        });

    assert_eq!(production_undo_result, Err("undo_mismatch"));
    assert_eq!(production_state, production_before);
    assert_eq!(model_undo_result, production_undo_result);
    assert_eq!(model_state, model_before);
}

#[test]
fn correspondence_pins_apply_undo_round_trip_restores_full_pre_state() {
    let previous = tagged_outpoint(0xb1);
    let ordinary = tagged_outpoint(0xb2);
    let transition = ReserveTransitionV1::new(ReserveTransitionV1Parts {
        chain_id: 7,
        height: 100,
        parent_block_hash: Hash256::from_bytes([0x10; 32]),
        previous: Some(ReservePoolSnapshotV1 {
            outpoint: previous,
            amount: 100,
        }),
        native_deposit_total: 30,
        execution_withdrawal_total: 10,
        execution_fee_total: 5,
        new_execution_balance_total: 115,
        producer_coinbase_txid: Hash256::from_bytes([0x20; 32]),
    })
    .unwrap();

    let mut production_state = UtxoState::try_from_entries([
        (previous, reserve_entry(100)),
        (ordinary, ordinary_entry(7)),
    ])
    .unwrap();
    let production_before = production_state.clone();
    let production_undo =
        crate::reserve::apply_reserve_transition_v1(&mut production_state, &transition).unwrap();
    assert_ne!(production_state, production_before);
    crate::reserve::undo_reserve_transition_v1(&mut production_state, &production_undo).unwrap();
    assert_eq!(production_state, production_before);

    let reserve = ModelSlot {
        key: 1,
        entry: ModelEntry {
            amount: 100,
            creation_height: 99,
            is_coinbase: false,
            program: ProgramClass::Reserve,
        },
    };
    let ordinary = ModelSlot {
        key: 2,
        entry: ModelEntry {
            amount: 7,
            creation_height: 88,
            is_coinbase: true,
            program: ProgramClass::Ordinary,
        },
    };
    let mut model_state = ModelState {
        slots: [Some(reserve), Some(ordinary), None, None],
    };
    let model_before = model_state;
    let model_undo = reserve_model::apply_state_with_undo(
        &mut model_state,
        StateTransition {
            previous: Some(StateSnapshot {
                key: 1,
                amount: 100,
            }),
            new_key: Some(7),
            new_amount: 115,
            height: 100,
        },
    )
    .unwrap();
    assert_ne!(model_state, model_before);
    reserve_model::undo_state(&mut model_state, &model_undo).unwrap();
    assert_eq!(model_state, model_before);
}
