use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use oregon_chainstate::{AcceptOutcome, ChainConfig, ChainState};
use oregon_consensus::{ConsensusParams, Target, params::KEY_COMMIT_V1};
use oregon_mempool::MempoolConfig;
use oregon_primitives::{
    Amount, Block, BlockHeader, FOUNDER_ALLOCATION_BASE_UNITS, Hash256, Transaction, TxInput,
    TxOutput, transaction_root, write_varint,
};
use oregon_utxo::{SpendVerifier, UtxoEntry, UtxoError};

use crate::core::spawn_core;

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "oregon-node-{label}-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

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

fn config() -> ChainConfig {
    let target = Target::from_le_bytes([0xff; 32]).unwrap();
    let genesis_timestamp = 1_800_000_000;
    ChainConfig {
        anchor_header: BlockHeader {
            version: 1,
            previous_block: Hash256::from_bytes([0; 32]),
            transaction_root: Hash256::from_bytes([0x22; 32]),
            timestamp: genesis_timestamp,
            difficulty_commitment: target.to_le_bytes(),
            nonce: 7,
        },
        genesis_timestamp,
        params: ConsensusParams::new(target, target, [0x42; 32]).unwrap(),
    }
}

fn height_one_founder_block(config: &ChainConfig, nonce: u64) -> Block {
    let mut height_bytes = Vec::new();
    write_varint(1, &mut height_bytes);
    let mut founder_program = vec![KEY_COMMIT_V1];
    founder_program.extend_from_slice(&config.params.founder_key_commitment);
    let coinbase = Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_txid: Hash256::from_bytes([0; 32]),
            previous_output_index: u32::MAX,
            sequence: u32::MAX,
            witness: vec![height_bytes],
        }],
        outputs: vec![TxOutput {
            value: Amount::from_base_units(FOUNDER_ALLOCATION_BASE_UNITS).unwrap(),
            locking_program: founder_program,
        }],
        lock_time: 0,
    };
    let transactions = vec![coinbase];
    Block {
        header: BlockHeader {
            version: 1,
            previous_block: config.anchor_header.block_id(),
            transaction_root: transaction_root(&transactions).unwrap(),
            timestamp: config.genesis_timestamp + 1,
            difficulty_commitment: config.params.initial_target.to_le_bytes(),
            nonce,
        },
        transactions,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn active_chain_change_reconciles_mempool_before_core_response_returns() {
    let dir = TestDir::new("active-reconcile");
    let config = config();
    let state = ChainState::open(dir.path(), config.clone()).unwrap();
    let core = spawn_core(state, MempoolConfig::default(), AcceptAllSpends);

    let before = core.test_snapshot().await.unwrap();
    assert_eq!(before.chain_base.tip_height, 0);

    let block = height_one_founder_block(&config, 101);
    let block_id = block.header.block_id();
    assert_eq!(
        core.submit_block(block).await.unwrap().unwrap(),
        AcceptOutcome::Extended
    );

    let after = core.test_snapshot().await.unwrap();
    assert_eq!(after.chain_base.tip_id, block_id);
    assert_eq!(after.chain_base.tip_height, 1);
    assert_eq!(after.mempool_len, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconciliation_failure_rebuilds_empty_pool_on_new_authoritative_base() {
    let dir = TestDir::new("reconcile-fallback");
    let config = config();
    let state = ChainState::open(dir.path(), config.clone()).unwrap();
    let mempool_config = MempoolConfig {
        max_entries: 7,
        max_total_bytes: 4096,
        max_ancestors: 3,
        max_descendants: 4,
    };
    let core = spawn_core(state, mempool_config, AcceptAllSpends);
    core.test_fail_next_reconcile().await.unwrap();

    let block = height_one_founder_block(&config, 102);
    let block_id = block.header.block_id();
    assert_eq!(
        core.submit_block(block).await.unwrap().unwrap(),
        AcceptOutcome::Extended
    );

    let after = core.test_snapshot().await.unwrap();
    assert_eq!(after.chain_base.tip_id, block_id);
    assert_eq!(after.chain_base.tip_height, 1);
    assert_eq!(after.mempool_len, 0);
    assert_eq!(after.test_rebuilds, 1);
}
