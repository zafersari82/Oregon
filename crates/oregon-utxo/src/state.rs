use std::collections::{HashMap, HashSet};

use oregon_primitives::{OutPoint, Transaction};

use crate::{SpendVerifier, UtxoEntry, UtxoError};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UtxoState {
    entries: HashMap<OutPoint, UtxoEntry>,
}

impl UtxoState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, outpoint: &OutPoint) -> Option<&UtxoEntry> {
        self.entries.get(outpoint)
    }

    #[cfg(test)]
    pub(crate) fn insert_test_utxo(&mut self, outpoint: OutPoint, entry: UtxoEntry) {
        self.entries.insert(outpoint, entry);
    }

    pub fn apply_normal_transaction<V: SpendVerifier>(
        &mut self,
        tx: &Transaction,
        spend_height: u64,
        verifier: &V,
    ) -> Result<u64, UtxoError> {
        let mut seen = HashSet::with_capacity(tx.inputs.len());
        let mut input_sum = 0u64;
        let mut consumed = Vec::with_capacity(tx.inputs.len());

        for (input_index, input) in tx.inputs.iter().enumerate() {
            let outpoint = input.outpoint();
            if !seen.insert(outpoint) {
                return Err(UtxoError::DuplicateInput(outpoint));
            }

            let entry = self
                .entries
                .get(&outpoint)
                .cloned()
                .ok_or(UtxoError::MissingUtxo(outpoint))?;

            if !entry.is_spendable_at(spend_height) {
                return Err(UtxoError::ImmatureCoinbase);
            }

            verifier.verify_spend(tx, input_index, &entry)?;
            input_sum = input_sum
                .checked_add(entry.output.value.base_units())
                .ok_or(UtxoError::AmountOverflow)?;
            consumed.push(outpoint);
        }

        let output_sum = tx.outputs.iter().try_fold(0u64, |sum, output| {
            sum.checked_add(output.value.base_units())
                .ok_or(UtxoError::AmountOverflow)
        })?;

        if output_sum > input_sum {
            return Err(UtxoError::OutputValueExceedsInput);
        }

        let fee = input_sum - output_sum;
        let txid = tx.txid();
        let mut created = Vec::with_capacity(tx.outputs.len());
        for (index, output) in tx.outputs.iter().enumerate() {
            let index = u32::try_from(index).map_err(|_| UtxoError::OutputIndexOverflow)?;
            let outpoint = OutPoint { txid, index };
            created.push((
                outpoint,
                UtxoEntry {
                    output: output.clone(),
                    creation_height: spend_height,
                    is_coinbase: false,
                },
            ));
        }

        for outpoint in consumed {
            self.entries.remove(&outpoint);
        }
        for (outpoint, entry) in created {
            self.entries.insert(outpoint, entry);
        }

        Ok(fee)
    }
}

#[cfg(test)]
mod coinbase_tests {
    use oregon_primitives::{
        Amount, FOUNDER_ALLOCATION_BASE_UNITS, Hash256, OutPoint, Transaction, TxInput, TxOutput,
    };

    use super::UtxoState;
    use crate::{SpendVerifier, UtxoEntry, UtxoError};

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

    fn coinbase(outputs: Vec<TxOutput>) -> Transaction {
        Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_txid: Hash256::from_bytes([0u8; 32]),
                previous_output_index: u32::MAX,
                sequence: u32::MAX,
                witness: vec![vec![1]],
            }],
            outputs,
            lock_time: 0,
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
            outputs: vec![TxOutput {
                value: Amount::from_base_units(value).unwrap(),
                locking_program: vec![0x01],
            }],
            lock_time: 0,
        }
    }

    #[test]
    fn founder_and_miner_outputs_share_coinbase_metadata_and_maturity() {
        let tx = coinbase(vec![
            TxOutput {
                value: Amount::from_base_units(FOUNDER_ALLOCATION_BASE_UNITS).unwrap(),
                locking_program: vec![0x01],
            },
            TxOutput {
                value: Amount::from_base_units(100).unwrap(),
                locking_program: vec![0x02],
            },
        ]);
        let mut state = UtxoState::new();
        state.insert_coinbase_outputs(&tx, 1).unwrap();

        for index in [0, 1] {
            let entry = state
                .get(&OutPoint {
                    txid: tx.txid(),
                    index,
                })
                .unwrap();
            assert!(entry.is_coinbase);
            assert_eq!(entry.creation_height, 1);
            assert!(!entry.is_spendable_at(120));
            assert!(entry.is_spendable_at(121));
        }
    }

    #[test]
    fn same_block_coinbase_spend_is_immature() {
        let tx = coinbase(vec![TxOutput {
            value: Amount::from_base_units(100).unwrap(),
            locking_program: vec![0x02],
        }]);
        let mut state = UtxoState::new();
        state.insert_coinbase_outputs(&tx, 10).unwrap();
        let previous = OutPoint {
            txid: tx.txid(),
            index: 0,
        };
        let spend = spend(previous, 90);

        assert_eq!(
            state.apply_normal_transaction(&spend, 10, &AcceptAll),
            Err(UtxoError::ImmatureCoinbase)
        );
    }
}
