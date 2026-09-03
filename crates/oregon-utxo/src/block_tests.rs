use oregon_consensus::{ConsensusError, ConsensusParams, Target, block_subsidy};
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

fn input(previous: OutPoint) -> TxInput {
    TxInput {
        previous_txid: previous.txid,
        previous_output_index: previous.index,
        sequence: 0,
        witness: vec![],
    }
}

fn spend(previous: OutPoint, value: u64) -> Transaction {
    Transaction {
        version: 1,
        inputs: vec![input(previous)],
        outputs: vec![output(value)],
        lock_time: 0,
    }
}

fn coinbase(height: u64, outputs: Vec<TxOutput>) -> Transaction {
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
        outputs,
        lock_time: 0,
    }
}

fn block(height: u64, transactions: Vec<Transaction>) -> Block {
    let root = transaction_root(&transactions).unwrap();
    Block {
        header: BlockHeader {
            version: 1,
            previous_block: Hash256::from_bytes([0x11; 32]),
            transaction_root: root,
            timestamp: 1_800_000_000 + height,
            difficulty_commitment: [0xff; 32],
            nonce: 1,
        },
        transactions,
    }
}

#[test]
fn same_block_parent_then_child_spend_connects_atomically() {
    let seed = OutPoint {
        txid: Hash256::from_bytes([0x31; 32]),
        index: 0,
    };
    let mut state = UtxoState::new();
    state.insert_test_utxo(seed, seed_entry(100));

    let parent = spend(seed, 90);
    let child_prev = OutPoint {
        txid: parent.txid(),
        index: 0,
    };
    let child = spend(child_prev, 80);
    let candidate = block(200, vec![coinbase(200, vec![]), parent, child.clone()]);

    let undo: BlockUndo = state
        .connect_block(&candidate, 200, &params(), &AcceptAll)
        .expect("topologically ordered block");

    assert!(state.get(&seed).is_none());
    assert!(
        state
            .get(&OutPoint {
                txid: child.txid(),
                index: 0,
            })
            .is_some()
    );
    assert!(!undo.spent.is_empty());
}

#[test]
fn child_before_parent_is_rejected_and_live_state_is_unchanged() {
    let seed = OutPoint {
        txid: Hash256::from_bytes([0x41; 32]),
        index: 0,
    };
    let mut state = UtxoState::new();
    state.insert_test_utxo(seed, seed_entry(100));
    let before = state.clone();

    let parent = spend(seed, 90);
    let child = spend(
        OutPoint {
            txid: parent.txid(),
            index: 0,
        },
        80,
    );
    let candidate = block(200, vec![coinbase(200, vec![]), child, parent]);

    assert_eq!(
        state.connect_block(&candidate, 200, &params(), &AcceptAll),
        Err(UtxoError::InvalidBlockOrder)
    );
    assert_eq!(state, before);
}

#[test]
fn double_spend_across_transactions_rejects_entire_block() {
    let seed = OutPoint {
        txid: Hash256::from_bytes([0x51; 32]),
        index: 0,
    };
    let mut state = UtxoState::new();
    state.insert_test_utxo(seed, seed_entry(100));
    let before = state.clone();

    let first = spend(seed, 90);
    let second = spend(seed, 80);
    let candidate = block(200, vec![coinbase(200, vec![]), first, second]);

    assert!(
        state
            .connect_block(&candidate, 200, &params(), &AcceptAll)
            .is_err()
    );
    assert_eq!(state, before);
}

#[test]
fn coinbase_claim_is_bound_to_exact_accumulated_fees() {
    let seed = OutPoint {
        txid: Hash256::from_bytes([0x61; 32]),
        index: 0,
    };
    let mut state = UtxoState::new();
    state.insert_test_utxo(seed, seed_entry(100));
    let before = state.clone();
    let normal = spend(seed, 90); // fee = 10
    let overclaim = block_subsidy(200).unwrap().base_units() + 11;
    let candidate = block(200, vec![coinbase(200, vec![output(overclaim)]), normal]);

    assert_eq!(
        state.connect_block(&candidate, 200, &params(), &AcceptAll),
        Err(UtxoError::Consensus(ConsensusError::CoinbaseOverClaim))
    );
    assert_eq!(state, before);
}

#[test]
fn final_invalid_transaction_rolls_back_all_earlier_overlay_changes() {
    let seed = OutPoint {
        txid: Hash256::from_bytes([0x71; 32]),
        index: 0,
    };
    let missing = OutPoint {
        txid: Hash256::from_bytes([0x72; 32]),
        index: 0,
    };
    let mut state = UtxoState::new();
    state.insert_test_utxo(seed, seed_entry(100));
    let before = state.clone();

    let valid = spend(seed, 90);
    let invalid = spend(missing, 1);
    let candidate = block(200, vec![coinbase(200, vec![]), valid, invalid]);

    assert!(
        state
            .connect_block(&candidate, 200, &params(), &AcceptAll)
            .is_err()
    );
    assert_eq!(state, before);
}
