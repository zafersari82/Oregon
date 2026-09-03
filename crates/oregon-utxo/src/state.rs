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
