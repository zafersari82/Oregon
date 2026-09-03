use oregon_consensus::{ConsensusError, validate_non_genesis_block_skeleton};
use oregon_primitives::{
    Amount, Block, BlockHeader, Hash256, Transaction, TxInput, TxOutput, transaction_root,
    write_varint,
};

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

fn normal(previous: Hash256) -> Transaction {
    Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_txid: previous,
            previous_output_index: 0,
            sequence: 0,
            witness: vec![],
        }],
        outputs: vec![TxOutput {
            value: Amount::from_base_units(1).unwrap(),
            locking_program: vec![1],
        }],
        lock_time: 0,
    }
}

fn block(height: u64, txs: Vec<Transaction>) -> Block {
    let root = transaction_root(&txs).unwrap();
    Block {
        header: BlockHeader {
            version: 1,
            previous_block: Hash256::from_bytes([0x11; 32]),
            transaction_root: root,
            timestamp: 1_800_000_000 + height,
            difficulty_commitment: [0xff; 32],
            nonce: 1,
        },
        transactions: txs,
    }
}

#[test]
fn skeleton_accepts_shape_without_knowing_fees() {
    let candidate = block(200, vec![coinbase(200), normal(Hash256::from_bytes([0x22; 32]))]);
    assert_eq!(validate_non_genesis_block_skeleton(&candidate, 200), Ok(()));
}

#[test]
fn skeleton_rejects_merkle_mutation_before_fee_accounting() {
    let mut candidate = block(200, vec![coinbase(200), normal(Hash256::from_bytes([0x22; 32]))]);
    candidate.header.transaction_root = Hash256::from_bytes([0x99; 32]);
    assert_eq!(
        validate_non_genesis_block_skeleton(&candidate, 200),
        Err(ConsensusError::MerkleRootMismatch)
    );
}
