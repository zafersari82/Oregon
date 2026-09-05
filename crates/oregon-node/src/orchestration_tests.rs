use oregon_chainstate::AcceptOutcome;
use oregon_mempool::{ChainBase, Mempool, MempoolConfig, MempoolError, ReconcileReport};
use oregon_primitives::{Amount, Block, BlockHeader, Hash256, OutPoint, Transaction, TxInput, TxOutput};
use oregon_utxo::{SpendVerifier, UtxoEntry, UtxoError, UtxoState};

use crate::orchestration::{reconcile_after_acceptance, recover_reconciliation_failure};

struct AcceptAllSpends;

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

fn base(tag: u8, height: u64) -> ChainBase {
    ChainBase {
        tip_id: Hash256::from_bytes([tag; 32]),
        tip_height: height,
    }
}

fn empty_block(tag: u8) -> Block {
    Block {
        header: BlockHeader {
            version: 1,
            previous_block: Hash256::from_bytes([tag; 32]),
            transaction_root: Hash256::from_bytes([tag.wrapping_add(1); 32]),
            timestamp: 1,
            difficulty_commitment: [0xff; 32],
            nonce: tag as u64,
        },
        transactions: Vec::new(),
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

fn spend(previous: OutPoint, value: u64) -> Transaction {
    Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_txid: previous.txid,
            previous_output_index: previous.index,
            sequence: 0,
            witness: Vec::new(),
        }],
        outputs: vec![TxOutput {
            value: Amount::from_base_units(value).unwrap(),
            locking_program: vec![0x51],
        }],
        lock_time: 0,
    }
}

#[test]
fn active_chain_change_reconciles_mempool_to_new_authoritative_base() {
    let old_base = base(0x10, 10);
    let new_base = base(0x11, 11);
    let config = MempoolConfig::default();
    let mut pool = Mempool::new(old_base, config.clone()).unwrap();
    let chain = UtxoState::default();

    let rebuilt = reconcile_after_acceptance(
        &mut pool,
        &config,
        AcceptOutcome::Extended,
        &empty_block(0x20),
        new_base,
        &chain,
        &AcceptAllSpends,
    );

    assert!(!rebuilt);
    assert_eq!(pool.base(), new_base);
}

#[test]
fn reconciliation_failure_rebuilds_empty_pool_on_new_authoritative_base() {
    let old_base = base(0x30, 30);
    let new_base = base(0x31, 31);
    let config = MempoolConfig {
        max_entries: 7,
        max_total_bytes: 4096,
        max_ancestors: 3,
        max_descendants: 4,
    };
    let root = OutPoint {
        txid: Hash256::from_bytes([0x40; 32]),
        index: 0,
    };
    let chain = UtxoState::try_from_entries(vec![(root, chain_entry(100))]).unwrap();
    let mut pool = Mempool::new(old_base, config.clone()).unwrap();
    pool.admit(spend(root, 90), old_base, &chain, &AcceptAllSpends)
        .unwrap();
    assert_eq!(pool.len(), 1);

    let rebuilt = recover_reconciliation_failure(
        &mut pool,
        &config,
        new_base,
        Err::<ReconcileReport, MempoolError>(MempoolError::InvariantViolation),
    );

    assert!(rebuilt);
    assert_eq!(pool.base(), new_base);
    assert_eq!(pool.len(), 0);
}
