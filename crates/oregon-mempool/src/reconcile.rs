#[cfg(test)]
mod tests {
    use oregon_primitives::{Block, BlockHeader, Hash256, OutPoint, Transaction, TxInput, TxOutput};
    use oregon_primitives::Amount;
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
        let chain = UtxoState::from_persisted_entries(vec![(root, chain_entry(100))]).unwrap();
        let old_base = base(0x72, 20);
        let mut pool = Mempool::new(old_base, MempoolConfig::default()).unwrap();
        let parent = transaction(root, 90, 1);
        let child_input = OutPoint {
            txid: parent.txid(),
            index: 0,
        };
        let child = transaction(child_input, 80, 2);
        pool.admit(parent.clone(), old_base, &chain, &Accept).unwrap();
        pool.admit(child.clone(), old_base, &chain, &Accept).unwrap();

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
}
