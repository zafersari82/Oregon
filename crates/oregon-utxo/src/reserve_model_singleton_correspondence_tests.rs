#[path = "../../../verification/reserve-conservation/src/model.rs"]
mod reserve_model;

use oregon_primitives::execution_reserve::EXECUTION_RESERVE_LOCKING_PROGRAM_V1;
use oregon_primitives::{Amount, Hash256, OutPoint, TxOutput};

use crate::{
    ReservePoolSnapshotV1, ReserveTransitionV1, ReserveTransitionV1Parts, UtxoEntry, UtxoState,
};
use reserve_model::{
    ModelEntry, ModelSlot, ModelState, ProgramClass, StateSnapshot, StateTransition,
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

fn transition(
    previous: Option<(OutPoint, u64)>,
    native_deposit_total: u64,
    execution_withdrawal_total: u64,
    execution_fee_total: u64,
    new_execution_balance_total: u64,
) -> ReserveTransitionV1 {
    ReserveTransitionV1::new(ReserveTransitionV1Parts {
        chain_id: 7,
        height: 100,
        parent_block_hash: Hash256::from_bytes([0x10; 32]),
        previous: previous.map(|(outpoint, amount)| ReservePoolSnapshotV1 { outpoint, amount }),
        native_deposit_total,
        execution_withdrawal_total,
        execution_fee_total,
        new_execution_balance_total,
        producer_coinbase_txid: Hash256::from_bytes([0x20; 32]),
    })
    .unwrap()
}

fn production_reserve_count(state: &UtxoState) -> usize {
    state
        .entries()
        .filter(|(_, entry)| {
            entry.output.locking_program.as_slice() == EXECUTION_RESERVE_LOCKING_PROGRAM_V1
        })
        .count()
}

fn model_reserve_count(state: &ModelState) -> usize {
    state
        .slots
        .iter()
        .flatten()
        .filter(|slot| slot.entry.program == ProgramClass::Reserve)
        .count()
}

#[test]
fn correspondence_pins_zero_result_has_no_live_reserve() {
    let previous = tagged_outpoint(0xd1);
    let ordinary = tagged_outpoint(0xd2);
    let transition = transition(Some((previous, 10)), 0, 10, 0, 0);

    let mut production_state = UtxoState::try_from_entries([
        (previous, reserve_entry(10)),
        (ordinary, ordinary_entry(7)),
    ])
    .unwrap();
    crate::reserve::apply_reserve_transition_v1(&mut production_state, &transition).unwrap();

    assert_eq!(transition.new_reserve_outpoint(), None);
    assert_eq!(production_reserve_count(&production_state), 0);
    assert_eq!(
        production_state.get(&ordinary),
        Some(&ordinary_entry(7)),
        "zero-result removal must not disturb unrelated ordinary entries"
    );

    let reserve_slot = ModelSlot {
        key: 1,
        entry: ModelEntry {
            amount: 10,
            creation_height: 99,
            is_coinbase: false,
            program: ProgramClass::Reserve,
        },
    };
    let ordinary_slot = ModelSlot {
        key: 2,
        entry: ModelEntry {
            amount: 7,
            creation_height: 88,
            is_coinbase: true,
            program: ProgramClass::Ordinary,
        },
    };
    let mut model_state = ModelState {
        slots: [Some(reserve_slot), Some(ordinary_slot), None, None],
    };
    reserve_model::apply_state(
        &mut model_state,
        StateTransition {
            previous: Some(StateSnapshot { key: 1, amount: 10 }),
            new_key: None,
            new_amount: 0,
            height: 100,
        },
    )
    .unwrap();

    assert_eq!(model_reserve_count(&model_state), 0);
    assert_eq!(
        model_reserve_count(&model_state),
        production_reserve_count(&production_state)
    );
    assert!(model_state.slots.contains(&Some(ordinary_slot)));
}

#[test]
fn correspondence_pins_positive_result_has_exactly_one_live_reserve() {
    let ordinary = tagged_outpoint(0xe1);
    let transition = transition(None, 10, 0, 0, 10);

    let mut production_state =
        UtxoState::try_from_entries([(ordinary, ordinary_entry(7))]).unwrap();
    crate::reserve::apply_reserve_transition_v1(&mut production_state, &transition).unwrap();

    assert!(transition.new_reserve_outpoint().is_some());
    assert_eq!(production_reserve_count(&production_state), 1);
    assert_eq!(
        production_state.get(&ordinary),
        Some(&ordinary_entry(7)),
        "positive-result creation must not disturb unrelated ordinary entries"
    );

    let ordinary_slot = ModelSlot {
        key: 2,
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
    reserve_model::apply_state(
        &mut model_state,
        StateTransition {
            previous: None,
            new_key: Some(7),
            new_amount: 10,
            height: 100,
        },
    )
    .unwrap();

    assert_eq!(model_reserve_count(&model_state), 1);
    assert_eq!(
        model_reserve_count(&model_state),
        production_reserve_count(&production_state)
    );
    assert!(model_state.slots.contains(&Some(ordinary_slot)));
}
