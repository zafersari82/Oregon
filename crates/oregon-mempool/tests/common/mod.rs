use oregon_mempool::ChainBase;
use oregon_primitives::{Amount, Hash256, OutPoint, Transaction, TxInput, TxOutput};
use oregon_utxo::{SpendVerifier, UtxoEntry, UtxoError, UtxoState};

pub struct AcceptTestSpends;

impl SpendVerifier for AcceptTestSpends {
    fn verify_spend(
        &self,
        _transaction: &Transaction,
        _input_index: usize,
        _prevout: &UtxoEntry,
    ) -> Result<(), UtxoError> {
        Ok(())
    }
}

pub struct RejectTestSpends;

impl SpendVerifier for RejectTestSpends {
    fn verify_spend(
        &self,
        _transaction: &Transaction,
        _input_index: usize,
        _prevout: &UtxoEntry,
    ) -> Result<(), UtxoError> {
        Err(UtxoError::SpendAuthorizationFailed)
    }
}

pub fn base(tag: u8, height: u64) -> ChainBase {
    ChainBase {
        tip_id: Hash256::from_bytes([tag; 32]),
        tip_height: height,
    }
}

pub fn outpoint(tag: u8, index: u32) -> OutPoint {
    OutPoint {
        txid: Hash256::from_bytes([tag; 32]),
        index,
    }
}

pub fn entry(value: u64, creation_height: u64, is_coinbase: bool) -> UtxoEntry {
    UtxoEntry {
        output: TxOutput {
            value: Amount::from_base_units(value).expect("test amount"),
            locking_program: vec![0x51],
        },
        creation_height,
        is_coinbase,
    }
}

pub fn spend(inputs: Vec<OutPoint>, outputs: &[u64], lock_time: u64) -> Transaction {
    Transaction {
        version: 1,
        inputs: inputs
            .into_iter()
            .map(|previous| TxInput {
                previous_txid: previous.txid,
                previous_output_index: previous.index,
                sequence: 0,
                witness: vec![],
            })
            .collect(),
        outputs: outputs
            .iter()
            .copied()
            .map(|value| TxOutput {
                value: Amount::from_base_units(value).expect("test amount"),
                locking_program: vec![0x51],
            })
            .collect(),
        lock_time,
    }
}

pub fn state_with(entries: Vec<(OutPoint, UtxoEntry)>) -> UtxoState {
    UtxoState::try_from_entries(entries).expect("valid test UTXO state")
}
