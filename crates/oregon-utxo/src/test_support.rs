use oregon_consensus::{ConsensusParams, Target};
use oregon_primitives::{
    Amount, Block, BlockHeader, Hash256, OutPoint, Transaction, TxInput, TxOutput, transaction_root,
};

use crate::{SpendVerifier, UtxoEntry, UtxoError};

pub(crate) struct AcceptAllSpends;

impl SpendVerifier for AcceptAllSpends {
    fn verify_spend(
        &self,
        _transaction: &Transaction,
        _input_index: usize,
        _prevout: &UtxoEntry,
    ) -> Result<(), UtxoError> {
        Ok(())
    }
}

pub(crate) fn consensus_params() -> ConsensusParams {
    ConsensusParams::new(
        Target::from_le_bytes([0xff; 32]).unwrap(),
        Target::from_le_bytes([0x7f; 32]).unwrap(),
        [0x42; 32],
    )
    .unwrap()
}

pub(crate) fn output(value: u64) -> TxOutput {
    TxOutput {
        value: Amount::from_base_units(value).unwrap(),
        locking_program: vec![0x01],
    }
}

pub(crate) fn seed_entry(value: u64) -> UtxoEntry {
    UtxoEntry {
        output: output(value),
        creation_height: 1,
        is_coinbase: false,
    }
}

pub(crate) fn spend(previous: OutPoint, value: u64) -> Transaction {
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

pub(crate) fn block(height: u64, transactions: Vec<Transaction>) -> Block {
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
