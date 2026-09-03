use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use oregon_consensus::{
    ChainWork, ConsensusError, ConsensusParams, HeaderContext, PowKeyBlockSource, Target,
    block_work, validate_header_pow, validate_header_pre_pow,
};
use oregon_pow::{LightEngine, derive_randomx_key, key_block_height};
use oregon_primitives::{Block, BlockHeader, Hash256, OutPoint, Transaction, transaction_root};
use oregon_storage::{BlockIndexRecord, OregonDb, StorageBatch, ValidationStatus};
use oregon_utxo::BlockUndo;

use crate::branch::BranchView;
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

fn seed_header_branch(
    path: &Path,
    config: &ChainConfig,
    tip_height: u64,
    nonce_domain: u64,
) -> Vec<Hash256> {
    drop(ChainState::open(path, config.clone()).unwrap());

    let db = OregonDb::open(path).unwrap();
    let mut ids = Vec::with_capacity(tip_height as usize + 1);
    ids.push(config.anchor_header.block_id());
    let mut parent = config.anchor_header.clone();
    let mut cumulative_work = ChainWork::zero();
    let per_block_work = block_work(config.params.initial_target);
    let mut batch = StorageBatch::new();

    for height in 1..=tip_height {
        let header = BlockHeader {
            version: 1,
            previous_block: parent.block_id(),
            transaction_root: Hash256::from_bytes([height as u8; 32]),
            timestamp: config.genesis_timestamp + height * 300,
            difficulty_commitment: config.params.initial_target.to_le_bytes(),
            nonce: nonce_domain + height,
        };
        cumulative_work.add_assign(&per_block_work);
        let block_id = header.block_id();
        batch.put_index(BlockIndexRecord {
            parent: header.previous_block,
            header: header.clone(),
            height,
            cumulative_work: cumulative_work.clone(),
            validation: ValidationStatus::HeaderValidated,
            body_retained: false,
        });
        ids.push(block_id);
        parent = header;
    }

    db.commit_durable(batch).unwrap();
    ids
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

#[test]
fn reopen_rejects_corrupt_persisted_utxo_bytes() {
    let dir = TestDir::new("corrupt-utxo");
    let config = test_config(1_800_000_000, 7);
    drop(ChainState::open(dir.path(), config.clone()).unwrap());

    let db = OregonDb::open_with_test_hooks(dir.path()).unwrap();
    let outpoint = OutPoint {
        txid: Hash256::from_bytes([0x55; 32]),
        index: 3,
    };
    db.test_put_raw_utxo_value(outpoint, &[0xff, 0x00]).unwrap();
    drop(db);

    assert!(ChainState::open(dir.path(), config).is_err());
}

#[test]
fn branch_view_follows_exact_candidate_ancestry_and_caps_mtp_at_eleven() {
    let dir = TestDir::new("branch-ancestry");
    let config = test_config(1_800_000_000, 7);
    let ids = seed_header_branch(dir.path(), &config, 13, 10_000);
    let db = OregonDb::open(dir.path()).unwrap();
    let view = BranchView::new(&db, ids[13]);

    assert_eq!(view.ancestor_id_at_height(0).unwrap(), Some(ids[0]));
    assert_eq!(view.ancestor_id_at_height(7).unwrap(), Some(ids[7]));
    assert_eq!(view.ancestor_id_at_height(13).unwrap(), Some(ids[13]));
    assert_eq!(view.ancestor_id_at_height(14).unwrap(), None);

    let expected: Vec<u64> = (3..=13)
        .rev()
        .map(|height| config.genesis_timestamp + height * 300)
        .collect();
    assert_eq!(view.mtp_window().unwrap(), expected);
}

#[test]
fn branch_view_supplies_randomx_key_block_from_candidate_ancestry() {
    let dir = TestDir::new("branch-randomx-key");
    let config = test_config(1_800_000_000, 7);
    let ids = seed_header_branch(dir.path(), &config, 887, 20_000);
    let db = OregonDb::open(dir.path()).unwrap();
    let view = BranchView::new(&db, ids[887]);

    assert_eq!(key_block_height(888), 864);
    assert_eq!(view.validated_block_id_at_height(864), Some(ids[864]));

    let parent = db.get_index(ids[887]).unwrap().unwrap();
    let mtp_window = view.mtp_window().unwrap();
    let candidate = BlockHeader {
        version: 1,
        previous_block: ids[887],
        transaction_root: Hash256::from_bytes([0x77; 32]),
        timestamp: config.genesis_timestamp + 888 * 300,
        difficulty_commitment: config.params.initial_target.to_le_bytes(),
        nonce: 30_000,
    };
    let facts = validate_header_pre_pow(
        &candidate,
        &HeaderContext {
            height: 888,
            parent: &parent.header,
            genesis_timestamp: config.genesis_timestamp,
            mtp_window: &mtp_window,
        },
        &config.params,
    )
    .unwrap();

    let mut correct_engine = LightEngine::new(derive_randomx_key(ids[864])).unwrap();
    validate_header_pow(&candidate, &facts, &view, &mut correct_engine).unwrap();

    let wrong_key_id = Hash256::from_bytes([0xee; 32]);
    assert_ne!(wrong_key_id, ids[864]);
    let mut wrong_engine = LightEngine::new(derive_randomx_key(wrong_key_id)).unwrap();
    assert_eq!(
        validate_header_pow(&candidate, &facts, &view, &mut wrong_engine),
        Err(ConsensusError::PowEngineKeyMismatch)
    );
}
