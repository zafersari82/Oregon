use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use oregon_consensus::{ChainWork, ConsensusParams, Target, block_work};
use oregon_primitives::{Block, BlockHeader, Hash256, Transaction, transaction_root};
use oregon_storage::{BlockIndexRecord, OregonDb, StorageBatch, ValidationStatus};
use oregon_utxo::BlockUndo;

use crate::{ChainConfig, ChainState, SessionHealth};

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "oregon-chainstate-{label}-{}-{n}",
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

fn test_anchor(genesis_timestamp: u64, nonce: u64) -> BlockHeader {
    BlockHeader {
        version: 1,
        previous_block: Hash256::from_bytes([0u8; 32]),
        transaction_root: Hash256::from_bytes([0x22; 32]),
        timestamp: genesis_timestamp,
        difficulty_commitment: [0xff; 32],
        nonce,
    }
}

fn test_config(genesis_timestamp: u64, nonce: u64) -> ChainConfig {
    let target = Target::from_le_bytes([0xff; 32]).unwrap();
    ChainConfig {
        anchor_header: test_anchor(genesis_timestamp, nonce),
        genesis_timestamp,
        params: ConsensusParams::new(target, target, [0x42; 32]).unwrap(),
    }
}

fn seed_height_one(
    path: &Path,
    config: &ChainConfig,
    body_retained: bool,
    prune_cursor: u64,
) -> Hash256 {
    drop(ChainState::open(path, config.clone()).unwrap());

    let transaction = Transaction {
        version: 1,
        inputs: vec![],
        outputs: vec![],
        lock_time: 1,
    };
    let transactions = vec![transaction];
    let header = BlockHeader {
        version: 1,
        previous_block: config.anchor_header.block_id(),
        transaction_root: transaction_root(&transactions).unwrap(),
        timestamp: config.genesis_timestamp + 1,
        difficulty_commitment: config.params.initial_target.to_le_bytes(),
        nonce: 11,
    };
    let block = Block {
        header: header.clone(),
        transactions,
    };
    let block_id = header.block_id();
    let index = BlockIndexRecord {
        header,
        parent: config.anchor_header.block_id(),
        height: 1,
        cumulative_work: block_work(config.params.initial_target),
        validation: ValidationStatus::FullyValidated,
        body_retained,
    };

    let db = OregonDb::open(path).unwrap();
    let mut batch = StorageBatch::new();
    batch.put_index(index);
    if body_retained {
        batch.put_block(block);
        batch.put_undo(
            block_id,
            BlockUndo {
                spent: vec![],
                created: vec![],
            },
        );
    }
    batch.set_active_height(1, block_id);
    batch.set_tip(block_id, 1);
    batch.set_prune_cursor(prune_cursor);
    db.commit_durable(batch).unwrap();
    block_id
}

#[test]
fn bootstrap_new_database_persists_zero_work_anchor_and_reopens_identically() {
    let dir = TestDir::new("bootstrap");
    let config = test_config(1_800_000_000, 7);
    let anchor_id = config.anchor_header.block_id();

    let state = ChainState::open(dir.path(), config.clone()).unwrap();
    assert_eq!(state.tip().block_id, anchor_id);
    assert_eq!(state.tip().height, 0);
    assert_eq!(state.tip().cumulative_work, ChainWork::zero());
    assert!(state.utxos().entries().next().is_none());
    assert_eq!(state.session_health(), SessionHealth::Healthy);
    drop(state);

    let reopened = ChainState::open(dir.path(), config).unwrap();
    assert_eq!(reopened.tip().block_id, anchor_id);
    assert_eq!(reopened.tip().height, 0);
    assert_eq!(reopened.tip().cumulative_work, ChainWork::zero());
    assert!(reopened.utxos().entries().next().is_none());
    assert_eq!(reopened.session_health(), SessionHealth::Healthy);
}

#[test]
fn reopen_fails_closed_if_anchor_or_genesis_timestamp_changes() {
    let dir = TestDir::new("config-binding");
    let original = test_config(1_800_000_000, 7);
    drop(ChainState::open(dir.path(), original.clone()).unwrap());

    assert!(ChainState::open(dir.path(), test_config(1_800_000_000, 8)).is_err());
    assert!(ChainState::open(dir.path(), test_config(1_800_000_001, 7)).is_err());

    let reopened = ChainState::open(dir.path(), original).unwrap();
    assert_eq!(reopened.tip().height, 0);
    assert_eq!(reopened.tip().cumulative_work, ChainWork::zero());
}

#[test]
fn reopen_rejects_prune_cursor_that_hides_required_rollback_data() {
    let dir = TestDir::new("prune-cursor-retention");
    let config = test_config(1_800_000_000, 7);
    seed_height_one(dir.path(), &config, false, 1);

    assert!(ChainState::open(dir.path(), config).is_err());
}

#[test]
fn reopen_accepts_intact_retained_height_one_state() {
    let dir = TestDir::new("retained-height-one");
    let config = test_config(1_800_000_000, 7);
    let block_id = seed_height_one(dir.path(), &config, true, 0);

    let reopened = ChainState::open(dir.path(), config).unwrap();
    assert_eq!(reopened.tip().block_id, block_id);
    assert_eq!(reopened.tip().height, 1);
    assert_eq!(
        reopened.tip().cumulative_work,
        block_work(Target::from_le_bytes([0xff; 32]).unwrap())
    );
}

#[test]
fn reopen_rejects_tampered_cumulative_chainwork() {
    let dir = TestDir::new("tampered-chainwork");
    let config = test_config(1_800_000_000, 7);
    let block_id = seed_height_one(dir.path(), &config, true, 0);

    let db = OregonDb::open(dir.path()).unwrap();
    let mut record = db.get_index(block_id).unwrap().unwrap();
    record.cumulative_work = ChainWork::zero();
    let mut batch = StorageBatch::new();
    batch.put_index(record);
    db.commit_durable(batch).unwrap();
    drop(db);

    assert!(ChainState::open(dir.path(), config).is_err());
}

#[test]
fn reopen_rejects_missing_active_mapping() {
    let dir = TestDir::new("missing-active-mapping");
    let config = test_config(1_800_000_000, 7);
    seed_height_one(dir.path(), &config, true, 0);

    let db = OregonDb::open(dir.path()).unwrap();
    let mut batch = StorageBatch::new();
    batch.delete_active_height(1);
    db.commit_durable(batch).unwrap();
    drop(db);

    assert!(ChainState::open(dir.path(), config).is_err());
}

#[test]
fn reopen_rejects_missing_retained_undo() {
    let dir = TestDir::new("missing-retained-undo");
    let config = test_config(1_800_000_000, 7);
    let block_id = seed_height_one(dir.path(), &config, true, 0);

    let db = OregonDb::open(dir.path()).unwrap();
    let mut batch = StorageBatch::new();
    batch.delete_undo(block_id);
    db.commit_durable(batch).unwrap();
    drop(db);

    assert!(ChainState::open(dir.path(), config).is_err());
}

#[test]
fn reopen_rejects_missing_retained_block_body() {
    let dir = TestDir::new("missing-retained-body");
    let config = test_config(1_800_000_000, 7);
    let block_id = seed_height_one(dir.path(), &config, true, 0);

    let db = OregonDb::open(dir.path()).unwrap();
    let mut batch = StorageBatch::new();
    batch.delete_block(block_id);
    db.commit_durable(batch).unwrap();
    drop(db);

    assert!(ChainState::open(dir.path(), config).is_err());
}
