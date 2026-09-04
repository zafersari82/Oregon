use oregon_primitives::{Hash256, OutPoint, Transaction, TxInput, write_varint};

use crate::test_support::{AcceptAllSpends, block, consensus_params, seed_entry, spend};
use crate::{BlockUndo, UtxoError, UtxoState};

fn coinbase(height: u64) -> Transaction {
    let mut height_bytes = Vec::new();
    write_varint(height, &mut height_bytes);
    Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_txid: Hash256::from_bytes([0; 32]),
            previous_output_index: u32::MAX,
            sequence: u32::MAX,
            witness: vec![height_bytes],
        }],
        outputs: vec![],
        lock_time: 0,
    }
}

#[test]
fn connect_then_disconnect_restores_exact_prior_state() {
    let seed = OutPoint {
        txid: Hash256::from_bytes([0x81; 32]),
        index: 0,
    };
    let mut state = UtxoState::new();
    state.insert_test_utxo(seed, seed_entry(100));
    let before = state.clone();
    let tx = spend(seed, 90);
    let candidate = block(200, vec![coinbase(200), tx]);

    let undo = state
        .connect_block(&candidate, 200, &consensus_params(), &AcceptAllSpends)
        .expect("connect");
    state.disconnect_block(undo).expect("disconnect");

    assert_eq!(state, before);
}

#[test]
fn same_block_intermediate_output_is_not_restored_by_disconnect() {
    let seed = OutPoint {
        txid: Hash256::from_bytes([0x82; 32]),
        index: 0,
    };
    let mut state = UtxoState::new();
    state.insert_test_utxo(seed, seed_entry(100));
    let before = state.clone();

    let parent = spend(seed, 90);
    let parent_outpoint = OutPoint {
        txid: parent.txid(),
        index: 0,
    };
    let child = spend(parent_outpoint, 80);
    let candidate = block(200, vec![coinbase(200), parent, child]);

    let undo = state
        .connect_block(&candidate, 200, &consensus_params(), &AcceptAllSpends)
        .expect("connect");
    state.disconnect_block(undo).expect("disconnect");

    assert_eq!(state, before);
    assert!(state.get(&parent_outpoint).is_none());
}

#[test]
fn tampered_undo_is_rejected_without_state_change() {
    let seed = OutPoint {
        txid: Hash256::from_bytes([0x83; 32]),
        index: 0,
    };
    let mut state = UtxoState::new();
    state.insert_test_utxo(seed, seed_entry(100));
    let tx = spend(seed, 90);
    let candidate = block(200, vec![coinbase(200), tx]);

    let mut undo = state
        .connect_block(&candidate, 200, &consensus_params(), &AcceptAllSpends)
        .expect("connect");
    let connected = state.clone();
    undo.created.push(OutPoint {
        txid: Hash256::from_bytes([0xee; 32]),
        index: 7,
    });

    assert_eq!(state.disconnect_block(undo), Err(UtxoError::UndoMismatch));
    assert_eq!(state, connected);
}

#[test]
fn duplicate_entries_inside_undo_are_rejected_atomically() {
    let outpoint = OutPoint {
        txid: Hash256::from_bytes([0x84; 32]),
        index: 0,
    };
    let mut state = UtxoState::new();
    state.insert_test_utxo(outpoint, seed_entry(100));
    let before = state.clone();
    let undo = BlockUndo {
        spent: vec![],
        created: vec![outpoint, outpoint],
    };

    assert_eq!(state.disconnect_block(undo), Err(UtxoError::UndoMismatch));
    assert_eq!(state, before);
}
