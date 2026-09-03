use oregon_consensus::{ConsensusParams, Target};
use oregon_primitives::{
    Amount, Block, BlockHeader, Hash256, OutPoint, Transaction, TxInput, TxOutput,
    transaction_root, write_varint,
};

use crate::{BlockUndo, SpendVerifier, UtxoEntry, UtxoError, UtxoState};

struct AcceptAll;

impl SpendVerifier for AcceptAll {
    fn verify_spend(
        &self,
        _transaction: &Transaction,
        _input_index: usize,
        _prevout: &UtxoEntry,
    ) -> Result<(), UtxoError> {
        Ok(())
    }
}

fn params() -> ConsensusParams {
    ConsensusParams::new(
        Target::from_le_bytes([0xff; 32]).unwrap(),
        Target::from_le_bytes([0x7f; 32]).unwrap(),
        [0x42; 32],
    )
    .unwrap()
}

fn output(value: u64) -> TxOutput {
    TxOutput {
        value: Amount::from_base_units(value).unwrap(),
        locking_program: vec![0x01],
    }
}

fn seed_entry(value: u64) -> UtxoEntry {
    UtxoEntry {
        output: output(value),
        creation_height: 1,
        is_coinbase: false,
    }
}

fn spend(previous: OutPoint, value: u64) -> Transaction {
    Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_txid: previous.txid,
            previous_output_index: previous.index,
            sequence: 0,
            witness: vec![],
        }],
        outputs: vec![output(value)],
        lock_time: 0,
    }
}

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

fn block(height: u64, transactions: Vec<Transaction>) -> Block {
    Block {
        header: BlockHeader {
            version: 1,
            previous_block: Hash256::from_bytes([0x11; 32]),
            transaction_root: transaction_root(&transactions).unwrap(),
            timestamp: 1_800_000_000 + height,
            difficulty_commitment: [0xff; 32],
            nonce: 1,
        },
        transactions,
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
        .connect_block(&candidate, 200, &params(), &AcceptAll)
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
        .connect_block(&candidate, 200, &params(), &AcceptAll)
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
        .connect_block(&candidate, 200, &params(), &AcceptAll)
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
