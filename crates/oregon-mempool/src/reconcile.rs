use std::collections::BTreeSet;

use oregon_primitives::{Block, Hash256, Transaction};
use oregon_utxo::{SpendVerifier, UtxoState};

use crate::graph::{descendant_closure, topological_order};
use crate::{ChainBase, Mempool, MempoolError, ReconcileReport};

impl Mempool {
    pub fn reconcile_active_block<V: SpendVerifier>(
        &mut self,
        block: &Block,
        new_base: ChainBase,
        chain_utxos: &UtxoState,
        verifier: &V,
    ) -> Result<ReconcileReport, MempoolError> {
        let order = topological_order(&self.entries)?;
        let confirmed: BTreeSet<_> = block.transactions.iter().map(Transaction::txid).collect();
        let mut excluded = BTreeSet::new();

        for transaction in &block.transactions {
            for input in &transaction.inputs {
                let outpoint = input.outpoint();
                let Some(conflicting_txid) = self.spenders.get(&outpoint).copied() else {
                    continue;
                };
                if confirmed.contains(&conflicting_txid) {
                    continue;
                }
                excluded.insert(conflicting_txid);
                excluded.extend(descendant_closure(&self.entries, conflicting_txid)?);
            }
        }

        let mut source = Vec::new();
        for txid in order {
            if confirmed.contains(&txid) || excluded.contains(&txid) {
                continue;
            }
            let entry = self
                .entries
                .get(&txid)
                .ok_or(MempoolError::InvariantViolation)?;
            source.push((txid, entry.transaction.clone()));
        }

        let rebuilt = self.rebuild_against_chain(&source, new_base, chain_utxos, verifier)?;
        let mut removed: Vec<_> = self
            .entries
            .keys()
            .filter(|txid| !rebuilt.entries.contains_key(txid))
            .copied()
            .collect();
        removed.sort();
        let retained = rebuilt.entries.len();

        *self = rebuilt;
        Ok(ReconcileReport { removed, retained })
    }

    pub fn reconcile_reorg<V: SpendVerifier>(
        &mut self,
        new_base: ChainBase,
        chain_utxos: &UtxoState,
        verifier: &V,
    ) -> Result<ReconcileReport, MempoolError> {
        let order = topological_order(&self.entries)?;
        let mut source = Vec::with_capacity(order.len());
        for txid in order {
            let entry = self
                .entries
                .get(&txid)
                .ok_or(MempoolError::InvariantViolation)?;
            source.push((txid, entry.transaction.clone()));
        }

        let rebuilt = self.rebuild_against_chain(&source, new_base, chain_utxos, verifier)?;
        let mut removed: Vec<_> = self
            .entries
            .keys()
            .filter(|txid| !rebuilt.entries.contains_key(txid))
            .copied()
            .collect();
        removed.sort();
        let retained = rebuilt.entries.len();

        *self = rebuilt;
        Ok(ReconcileReport { removed, retained })
    }

    pub(crate) fn rebuild_against_chain<V: SpendVerifier>(
        &self,
        ordered_source: &[(Hash256, Transaction)],
        new_base: ChainBase,
        chain_utxos: &UtxoState,
        verifier: &V,
    ) -> Result<Mempool, MempoolError> {
        let mut rebuilt = Mempool::new(new_base, self.config.clone())?;

        for (expected_txid, transaction) in ordered_source {
            if transaction.txid() != *expected_txid {
                return Err(MempoolError::InvariantViolation);
            }

            match rebuilt.admit(transaction.clone(), new_base, chain_utxos, verifier) {
                Ok(_) => {}
                Err(error) if is_filterable_rebuild_error(&error) => {}
                Err(error) => return Err(error),
            }
        }

        Ok(rebuilt)
    }
}

fn is_filterable_rebuild_error(error: &MempoolError) -> bool {
    matches!(
        error,
        MempoolError::MissingDependency(_)
            | MempoolError::InvalidParentOutput(_)
            | MempoolError::TooManyAncestors
            | MempoolError::TooManyDescendants
            | MempoolError::CapacityRejected
            | MempoolError::Structural(_)
            | MempoolError::Utxo(_)
    )
}

#[cfg(test)]
mod tests {
    use oregon_primitives::Amount;
    use oregon_primitives::{
        Block, BlockHeader, Hash256, OutPoint, Transaction, TxInput, TxOutput,
    };
    use oregon_utxo::{SpendVerifier, UtxoEntry, UtxoError, UtxoState};

    use crate::{ChainBase, Mempool, MempoolConfig, MempoolError};

    struct Accept;

    impl SpendVerifier for Accept {
        fn verify_spend(
            &self,
            _transaction: &Transaction,
            _input_index: usize,
            _prevout: &UtxoEntry,
        ) -> Result<(), UtxoError> {
            Ok(())
        }
    }

    fn base(tag: u8, height: u64) -> ChainBase {
        ChainBase {
            tip_id: Hash256::from_bytes([tag; 32]),
            tip_height: height,
        }
    }

    fn transaction(previous: OutPoint, value: u64, lock_time: u64) -> Transaction {
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
                locking_program: vec![0x51],
            }],
            lock_time,
        }
    }

    fn chain_entry(value: u64) -> UtxoEntry {
        UtxoEntry {
            output: TxOutput {
                value: Amount::from_base_units(value).unwrap(),
                locking_program: vec![0x51],
            },
            creation_height: 1,
            is_coinbase: false,
        }
    }

    #[test]
    fn reconciliation_invariant_failure_preserves_live_pool_exactly() {
        let root = OutPoint {
            txid: Hash256::from_bytes([0x71; 32]),
            index: 0,
        };
        let chain = UtxoState::try_from_entries(vec![(root, chain_entry(100))]).unwrap();
        let old_base = base(0x72, 20);
        let mut pool = Mempool::new(old_base, MempoolConfig::default()).unwrap();
        let parent = transaction(root, 90, 1);
        let child_input = OutPoint {
            txid: parent.txid(),
            index: 0,
        };
        let child = transaction(child_input, 80, 2);
        pool.admit(parent.clone(), old_base, &chain, &Accept)
            .unwrap();
        pool.admit(child.clone(), old_base, &chain, &Accept)
            .unwrap();

        pool.entries
            .get_mut(&parent.txid())
            .unwrap()
            .children
            .clear();
        let before_base = pool.base;
        let before_entries = pool.entries.clone();
        let before_spenders = pool.spenders.clone();
        let before_bytes = pool.total_bytes;

        let block = Block {
            header: BlockHeader {
                version: 1,
                previous_block: Hash256::from_bytes([0x73; 32]),
                transaction_root: Hash256::from_bytes([0x74; 32]),
                timestamp: 1,
                difficulty_commitment: [0x75; 32],
                nonce: 1,
            },
            transactions: vec![],
        };
        let new_base = base(0x76, 21);

        assert_eq!(
            pool.reconcile_active_block(&block, new_base, &chain, &Accept),
            Err(MempoolError::InvariantViolation)
        );
        assert_eq!(pool.base, before_base);
        assert_eq!(pool.entries, before_entries);
        assert_eq!(pool.spenders, before_spenders);
        assert_eq!(pool.total_bytes, before_bytes);
    }

    #[test]
    fn reorg_dependency_cycle_preserves_live_pool_exactly() {
        let root = OutPoint {
            txid: Hash256::from_bytes([0x81; 32]),
            index: 0,
        };
        let chain = UtxoState::try_from_entries(vec![(root, chain_entry(100))]).unwrap();
        let old_base = base(0x82, 20);
        let mut pool = Mempool::new(old_base, MempoolConfig::default()).unwrap();
        let parent = transaction(root, 90, 1);
        let child = transaction(
            OutPoint {
                txid: parent.txid(),
                index: 0,
            },
            80,
            2,
        );
        let parent_txid = parent.txid();
        let child_txid = child.txid();
        pool.admit(parent, old_base, &chain, &Accept).unwrap();
        pool.admit(child, old_base, &chain, &Accept).unwrap();

        pool.entries
            .get_mut(&parent_txid)
            .unwrap()
            .parents
            .insert(child_txid);
        pool.entries
            .get_mut(&child_txid)
            .unwrap()
            .children
            .insert(parent_txid);

        let before_base = pool.base;
        let before_entries = pool.entries.clone();
        let before_spenders = pool.spenders.clone();
        let before_bytes = pool.total_bytes;

        assert_eq!(
            pool.reconcile_reorg(base(0x83, 19), &chain, &Accept),
            Err(MempoolError::DependencyCycle)
        );
        assert_eq!(pool.base, before_base);
        assert_eq!(pool.entries, before_entries);
        assert_eq!(pool.spenders, before_spenders);
        assert_eq!(pool.total_bytes, before_bytes);
    }
}
