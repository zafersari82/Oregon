use oregon_primitives::execution_reserve::EXECUTION_RESERVE_LOCKING_PROGRAM_V1;
use oregon_primitives::{Amount, Hash256, OutPoint, TxOutput};

use crate::reserve_model;
use crate::{
    ReservePoolSnapshotV1, ReserveTransitionError, ReserveTransitionV1, ReserveTransitionV1Parts,
    UtxoEntry, UtxoState,
};
use reserve_model::{
    ArithmeticInput, ModelEntry, ModelSlot, ModelState, ProgramClass, StateError, StateSnapshot,
    StateTransition,
};

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

fn production_error(error: ReserveTransitionError) -> &'static str {
    match error {
        ReserveTransitionError::OutputCollision(_) => "output_collision",
        ReserveTransitionError::MultipleLiveReserves => "multiple_live_reserves",
        ReserveTransitionError::PreviousReserveMismatch => "previous_reserve_mismatch",
        ReserveTransitionError::UndoMismatch => "undo_mismatch",
        _ => "other_state_error",
    }
}

fn model_error(error: StateError) -> &'static str {
    match error {
        StateError::OutputCollision => "output_collision",
        StateError::MultipleLiveReserves => "multiple_live_reserves",
        StateError::PreviousReserveMismatch => "previous_reserve_mismatch",
        StateError::UndoMismatch => "undo_mismatch",
    }
}

fn assert_shared_model_source_is_live() {
    assert_eq!(
        reserve_model::construct_arithmetic(ArithmeticInput {
            previous: None,
            native_deposit_total: 1,
            execution_withdrawal_total: 0,
            execution_fee_total: 0,
            new_execution_balance_total: 1,
        }),
        Ok(1)
    );
}

#[test]
fn correspondence_pins_output_collision_apply_rejection_and_atomicity() {
    assert_shared_model_source_is_live();

    let previous = tagged_outpoint(0xf1);
    let unrelated = tagged_outpoint(0xf2);
    let transition = ReserveTransitionV1::new(ReserveTransitionV1Parts {
        chain_id: 7,
        height: 100,
        parent_block_hash: Hash256::from_bytes([0x10; 32]),
        previous: Some(ReservePoolSnapshotV1 {
            outpoint: previous,
            amount: 100,
        }),
        native_deposit_total: 10,
        execution_withdrawal_total: 0,
        execution_fee_total: 0,
        new_execution_balance_total: 110,
        producer_coinbase_txid: Hash256::from_bytes([0x20; 32]),
    })
    .unwrap();
    let collision = transition.new_reserve_outpoint().unwrap();

    let mut production_state = UtxoState::try_from_entries([
        (previous, reserve_entry(100)),
        (collision, ordinary_entry(7)),
        (unrelated, ordinary_entry(9)),
    ])
    .unwrap();
    let production_before = production_state.clone();
    let production_apply =
        crate::reserve::apply_reserve_transition_v1(&mut production_state, &transition)
            .map(|_| ())
            .map_err(production_error);

    assert_eq!(production_apply, Err("output_collision"));
    assert_eq!(production_state, production_before);

    let reserve_slot = ModelSlot {
        key: 1,
        entry: ModelEntry {
            amount: 100,
            creation_height: 99,
            is_coinbase: false,
            program: ProgramClass::Reserve,
        },
    };
    let collision_slot = ModelSlot {
        key: 7,
        entry: ModelEntry {
            amount: 7,
            creation_height: 88,
            is_coinbase: true,
            program: ProgramClass::Ordinary,
        },
    };
    let unrelated_slot = ModelSlot {
        key: 3,
        entry: ModelEntry {
            amount: 9,
            creation_height: 77,
            is_coinbase: false,
            program: ProgramClass::Ordinary,
        },
    };
    let mut model_state = ModelState {
        slots: [
            Some(reserve_slot),
            Some(collision_slot),
            Some(unrelated_slot),
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
            new_key: Some(7),
            new_amount: 110,
            height: 100,
        },
    )
    .map_err(model_error);

    assert_eq!(model_apply, production_apply);
    assert_eq!(model_state, model_before);
}

#[test]
fn correspondence_pins_amount_tampered_undo_rejection_and_atomicity() {
    let ordinary = tagged_outpoint(0xf3);
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
    tampered_entries
        .iter_mut()
        .find(|(outpoint, _)| *outpoint == created)
        .map(|(_, entry)| entry)
        .unwrap()
        .output
        .value = Amount::from_base_units(11).unwrap();
    production_state = UtxoState::try_from_entries(tampered_entries).unwrap();
    let production_before = production_state.clone();
    let production_undo_result =
        crate::reserve::undo_reserve_transition_v1(&mut production_state, &production_undo)
            .map_err(production_error);

    assert_eq!(production_undo_result, Err("undo_mismatch"));
    assert_eq!(production_state, production_before);

    let ordinary_slot = ModelSlot {
        key: 3,
        entry: ModelEntry {
            amount: 7,
            creation_height: 88,
            is_coinbase: true,
            program: ProgramClass::Ordinary,
        },
    };
    let mut model_state = ModelState {
        slots: [Some(ordinary_slot), None, None, None],
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
        .amount = 11;
    let model_before = model_state;
    let model_undo_result =
        reserve_model::undo_state(&mut model_state, &model_undo).map_err(model_error);

    assert_eq!(model_undo_result, production_undo_result);
    assert_eq!(model_state, model_before);
    assert_eq!(production_state.get(&ordinary), Some(&ordinary_entry(7)));
    assert!(model_state.slots.contains(&Some(ordinary_slot)));
}
