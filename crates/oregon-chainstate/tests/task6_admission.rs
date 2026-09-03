use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use oregon_chainstate::{AcceptOutcome, ChainConfig, ChainState, ChainStateError};
use oregon_consensus::{ConsensusParams, Target, block_work};
use oregon_primitives::{Block, BlockHeader, Hash256, Transaction, transaction_root};
use oregon_storage::{BlockIndexRecord, OregonDb, StorageBatch, ValidationStatus};
use oregon_utxo::{BlockUndo, SpendVerifier, UtxoEntry, UtxoError};

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("oregon-task6-{label}-{}-{n}", std::process::id()));
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

struct RejectTestSpends;

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

fn block(parent: Hash256, config: &ChainConfig, timestamp_offset: u64, nonce: u64) -> Block {
    let transactions = vec![Transaction {
        version: 1,
        inputs: vec![],
        outputs: vec![],
        lock_time: nonce,
    }];
    Block {
        header: BlockHeader {
            version: 1,
            previous_block: parent,
            transaction_root: transaction_root(&transactions).unwrap(),
            timestamp: config.genesis_timestamp + timestamp_offset,
            difficulty_commitment: config.params.initial_target.to_le_bytes(),
            nonce,
        },
        transactions,
    }
}

fn seed_active_height_one(path: &Path, config: &ChainConfig) -> Hash256 {
    drop(ChainState::open(path, config.clone()).unwrap());

    let active = block(config.anchor_header.block_id(), config, 1, 11);
    let active_id = active.header.block_id();
    let mut batch = StorageBatch::new();
    batch.put_index(BlockIndexRecord {
        header: active.header.clone(),
        parent: config.anchor_header.block_id(),
        height: 1,
        cumulative_work: block_work(config.params.initial_target),
        validation: ValidationStatus::FullyValidated,
        body_retained: true,
    });
    batch.put_block(active);
    batch.put_undo(
        active_id,
        BlockUndo {
            spent: vec![],
            created: vec![],
        },
    );
    batch.set_active_height(1, active_id);
    batch.set_tip(active_id, 1);
    OregonDb::open(path).unwrap().commit_durable(batch).unwrap();
    active_id
}

#[test]
fn known_side_chain_block_is_idempotent() {
    let dir = TestDir::new("idempotent");
    let config = config();
    let active_id = seed_active_height_one(dir.path(), &config);
    let candidate = block(config.anchor_header.block_id(), &config, 300, 99);
    let candidate_id = candidate.header.block_id();

    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();
    let before_tip = state.tip().clone();
    assert_eq!(
        state
            .accept_block(candidate.clone(), &RejectTestSpends)
            .unwrap(),
        AcceptOutcome::StoredSideChain
    );
    assert_eq!(
        state
            .accept_block(candidate.clone(), &RejectTestSpends)
            .unwrap(),
        AcceptOutcome::StoredSideChain
    );
    assert_eq!(state.tip(), &before_tip);
    drop(state);

    let db = OregonDb::open(dir.path()).unwrap();
    let index = db.get_index(candidate_id).unwrap().unwrap();
    assert_eq!(index.validation, ValidationStatus::HeaderValidated);
    assert_eq!(
        index.cumulative_work,
        block_work(config.params.initial_target)
    );
    assert_eq!(db.get_block(candidate_id).unwrap(), Some(candidate));
    assert_eq!(db.active_tip().unwrap(), Some((active_id, 1)));
}

#[test]
fn descendant_of_invalid_parent_is_rejected_without_persistence() {
    let dir = TestDir::new("invalid-parent");
    let config = config();
    seed_active_height_one(dir.path(), &config);

    let invalid_parent = block(config.anchor_header.block_id(), &config, 300, 101);
    let invalid_parent_id = invalid_parent.header.block_id();
    let mut batch = StorageBatch::new();
    batch.put_index(BlockIndexRecord {
        header: invalid_parent.header,
        parent: config.anchor_header.block_id(),
        height: 1,
        cumulative_work: block_work(config.params.initial_target),
        validation: ValidationStatus::Invalid,
        body_retained: false,
    });
    OregonDb::open(dir.path())
        .unwrap()
        .commit_durable(batch)
        .unwrap();

    let candidate = block(invalid_parent_id, &config, 600, 102);
    let candidate_id = candidate.header.block_id();
    let mut state = ChainState::open(dir.path(), config).unwrap();
    assert!(matches!(
        state.accept_block(candidate, &RejectTestSpends),
        Err(ChainStateError::CorruptState(_))
    ));
    drop(state);

    let db = OregonDb::open(dir.path()).unwrap();
    assert_eq!(db.get_index(candidate_id).unwrap(), None);
    assert_eq!(db.get_block(candidate_id).unwrap(), None);
}
