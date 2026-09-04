use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use oregon_consensus::{ChainWork, ConsensusParams, Target, block_work};
use oregon_primitives::{
    Amount, Block, BlockHeader, FOUNDER_ALLOCATION_BASE_UNITS, Hash256, OutPoint, Transaction,
    TxInput, TxOutput, transaction_root, write_varint,
};
use oregon_storage::{BlockIndexRecord, OregonDb, StorageBatch, ValidationStatus};
use oregon_utxo::{BlockUndo, SpendVerifier, UtxoEntry, UtxoError};

use crate::state::REORG_WINDOW;
use crate::{AcceptOutcome, ChainConfig, ChainState, SessionHealth};

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "oregon-chainstate-recovery-{label}-{}-{n}",
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

fn test_config() -> ChainConfig {
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

fn coinbase(config: &ChainConfig, height: u64) -> Transaction {
    let mut height_bytes = Vec::new();
    write_varint(height, &mut height_bytes);
    let outputs = if height == 1 {
        let mut founder_program = vec![0x01];
        founder_program.extend_from_slice(&config.params.founder_key_commitment);
        vec![TxOutput {
            value: Amount::from_base_units(FOUNDER_ALLOCATION_BASE_UNITS).unwrap(),
            locking_program: founder_program,
        }]
    } else {
        vec![TxOutput {
            value: Amount::from_base_units(1).unwrap(),
            locking_program: vec![0x51],
        }]
    };

    Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_txid: Hash256::from_bytes([0; 32]),
            previous_output_index: u32::MAX,
            sequence: u32::MAX,
            witness: vec![height_bytes],
        }],
        outputs,
        lock_time: 0,
    }
}

fn spend(previous: OutPoint, output_value: u64) -> Transaction {
    Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_txid: previous.txid,
            previous_output_index: previous.index,
            sequence: 0,
            witness: vec![],
        }],
        outputs: vec![TxOutput {
            value: Amount::from_base_units(output_value).unwrap(),
            locking_program: vec![0x51, 0x21],
        }],
        lock_time: 0,
    }
}

fn block(
    config: &ChainConfig,
    parent: Hash256,
    height: u64,
    nonce: u64,
    transactions: Vec<Transaction>,
) -> Block {
    let root = transaction_root(&transactions).unwrap();
    Block {
        header: BlockHeader {
            version: 1,
            previous_block: parent,
            transaction_root: root,
            timestamp: config.genesis_timestamp + height * 300,
            difficulty_commitment: config.params.initial_target.to_le_bytes(),
            nonce,
        },
        transactions,
    }
}

fn sorted_utxos(state: &ChainState) -> Vec<(OutPoint, UtxoEntry)> {
    let mut entries: Vec<_> = state
        .utxos()
        .entries()
        .map(|(outpoint, entry)| (*outpoint, entry.clone()))
        .collect();
    entries.sort_by_key(|(outpoint, _)| *outpoint);
    entries
}

fn seed_spendable_utxo(path: &Path, config: &ChainConfig) -> OutPoint {
    drop(ChainState::open(path, config.clone()).unwrap());
    let outpoint = OutPoint {
        txid: Hash256::from_bytes([0xa5; 32]),
        index: 3,
    };
    let entry = UtxoEntry {
        output: TxOutput {
            value: Amount::from_base_units(100).unwrap(),
            locking_program: vec![0x51],
        },
        creation_height: 0,
        is_coinbase: false,
    };
    let db = OregonDb::open(path).unwrap();
    let mut batch = StorageBatch::new();
    batch.put_utxo(outpoint, entry);
    db.commit_durable(batch).unwrap();
    outpoint
}

fn seed_storage_chain(
    path: &Path,
    config: &ChainConfig,
    tip_height: u64,
    retain_extra_old_body: bool,
) -> Vec<Hash256> {
    drop(ChainState::open(path, config.clone()).unwrap());
    let db = OregonDb::open(path).unwrap();
    let per_block_work = block_work(config.params.initial_target);
    let safe_prune_cursor = tip_height.saturating_sub(REORG_WINDOW);
    let persisted_cursor = if retain_extra_old_body {
        safe_prune_cursor.saturating_sub(1)
    } else {
        safe_prune_cursor
    };

    let mut ids = Vec::with_capacity(tip_height as usize + 1);
    let anchor_id = config.anchor_header.block_id();
    ids.push(anchor_id);
    let mut parent_id = anchor_id;
    let mut cumulative_work = ChainWork::zero();
    let mut batch = StorageBatch::new();

    for height in 1..=tip_height {
        let transactions = vec![coinbase(config, height)];
        let header = BlockHeader {
            version: 1,
            previous_block: parent_id,
            transaction_root: transaction_root(&transactions).unwrap(),
            timestamp: config.genesis_timestamp + height * 300,
            difficulty_commitment: config.params.initial_target.to_le_bytes(),
            nonce: 100_000 + height,
        };
        let block_id = header.block_id();
        cumulative_work.add_assign(&per_block_work);
        let body_retained = height > safe_prune_cursor
            || (retain_extra_old_body && height == safe_prune_cursor && safe_prune_cursor > 0);

        batch.put_index(BlockIndexRecord {
            header: header.clone(),
            parent: parent_id,
            height,
            cumulative_work: cumulative_work.clone(),
            validation: ValidationStatus::FullyValidated,
            body_retained,
        });
        if body_retained {
            batch.put_block(Block {
                header,
                transactions,
            });
            batch.put_undo(
                block_id,
                BlockUndo {
                    spent: vec![],
                    created: vec![],
                },
            );
        }
        batch.set_active_height(height, block_id);
        ids.push(block_id);
        parent_id = block_id;
    }

    batch.set_tip(parent_id, tip_height);
    batch.set_prune_cursor(persisted_cursor);
    db.commit_durable(batch).unwrap();
    ids
}

#[test]
fn multiple_blocks_with_a_spend_reopen_to_identical_tip_and_sorted_utxos() {
    let dir = TestDir::new("multi-block-reopen");
    let config = test_config();
    let seeded = seed_spendable_utxo(dir.path(), &config);
    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();

    let block1 = block(
        &config,
        config.anchor_header.block_id(),
        1,
        101,
        vec![coinbase(&config, 1), spend(seeded, 90)],
    );
    assert_eq!(
        state
            .accept_block(block1.clone(), &AcceptAllSpends)
            .unwrap(),
        AcceptOutcome::Extended
    );
    let block2 = block(
        &config,
        block1.header.block_id(),
        2,
        102,
        vec![coinbase(&config, 2)],
    );
    assert_eq!(
        state.accept_block(block2, &AcceptAllSpends).unwrap(),
        AcceptOutcome::Extended
    );

    let expected_tip = state.tip().clone();
    let expected_utxos = sorted_utxos(&state);
    drop(state);

    let reopened = ChainState::open(dir.path(), config).unwrap();
    assert_eq!(reopened.tip(), &expected_tip);
    assert_eq!(sorted_utxos(&reopened), expected_utxos);
    assert_eq!(reopened.session_health(), SessionHealth::Healthy);
}

#[test]
fn accepted_active_state_reopens_when_pruning_was_skipped_and_side_data_remains() {
    let dir = TestDir::new("accepted-before-prune");
    let config = test_config();
    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();

    let active = block(
        &config,
        config.anchor_header.block_id(),
        1,
        201,
        vec![coinbase(&config, 1)],
    );
    assert_eq!(
        state
            .accept_block(active.clone(), &AcceptAllSpends)
            .unwrap(),
        AcceptOutcome::Extended
    );

    let side_transactions = vec![coinbase(&config, 1)];
    let side = Block {
        header: BlockHeader {
            version: 1,
            previous_block: config.anchor_header.block_id(),
            transaction_root: transaction_root(&side_transactions).unwrap(),
            timestamp: config.genesis_timestamp + 300,
            difficulty_commitment: config.params.initial_target.to_le_bytes(),
            nonce: 202,
        },
        transactions: side_transactions,
    };
    let side_id = side.header.block_id();
    assert_eq!(
        state.accept_block(side, &AcceptAllSpends).unwrap(),
        AcceptOutcome::StoredSideChain
    );
    let expected_tip = state.tip().clone();
    drop(state);

    let reopened = ChainState::open(dir.path(), config).unwrap();
    assert_eq!(reopened.tip(), &expected_tip);
    drop(reopened);

    let db = OregonDb::open(dir.path()).unwrap();
    assert!(db.get_block(side_id).unwrap().is_some());
    assert_eq!(
        db.active_tip().unwrap(),
        Some((active.header.block_id(), 1))
    );
}

#[test]
fn skipped_pruning_with_extra_old_body_is_harmless_on_reopen() {
    let dir = TestDir::new("skipped-prune-old-body");
    let config = test_config();
    let ids = seed_storage_chain(dir.path(), &config, REORG_WINDOW + 1, true);
    let old_id = ids[1];

    let db = OregonDb::open(dir.path()).unwrap();
    assert_eq!(db.prune_cursor().unwrap(), Some(0));
    assert!(db.get_index(old_id).unwrap().unwrap().body_retained);
    assert!(db.get_block(old_id).unwrap().is_some());
    assert!(db.get_undo(old_id).unwrap().is_some());
    drop(db);

    let reopened = ChainState::open(dir.path(), config).unwrap();
    assert_eq!(reopened.tip().height, REORG_WINDOW + 1);
    assert_eq!(reopened.tip().block_id, *ids.last().unwrap());
    assert_eq!(reopened.session_health(), SessionHealth::Healthy);
}

#[test]
fn missing_body_behind_valid_prune_horizon_reopens_healthy() {
    let dir = TestDir::new("pruned-old-body");
    let config = test_config();
    let ids = seed_storage_chain(dir.path(), &config, REORG_WINDOW + 1, false);
    let old_id = ids[1];

    let db = OregonDb::open(dir.path()).unwrap();
    assert_eq!(db.prune_cursor().unwrap(), Some(1));
    assert!(!db.get_index(old_id).unwrap().unwrap().body_retained);
    assert!(db.get_block(old_id).unwrap().is_none());
    assert!(db.get_undo(old_id).unwrap().is_none());
    drop(db);

    let reopened = ChainState::open(dir.path(), config).unwrap();
    assert_eq!(reopened.tip().height, REORG_WINDOW + 1);
    assert_eq!(reopened.tip().block_id, *ids.last().unwrap());
    assert_eq!(reopened.session_health(), SessionHealth::Healthy);
}

#[test]
fn reopen_fails_closed_when_active_index_height_is_tampered() {
    let dir = TestDir::new("index-height-corruption");
    let config = test_config();
    let ids = seed_storage_chain(dir.path(), &config, 2, false);

    let db = OregonDb::open(dir.path()).unwrap();
    let mut record = db.get_index(ids[1]).unwrap().unwrap();
    record.height = 9;
    let mut batch = StorageBatch::new();
    batch.put_index(record);
    db.commit_durable(batch).unwrap();
    drop(db);

    assert!(ChainState::open(dir.path(), config).is_err());
}

#[test]
fn reopen_fails_closed_when_active_parent_link_is_tampered() {
    let dir = TestDir::new("index-parent-corruption");
    let config = test_config();
    let ids = seed_storage_chain(dir.path(), &config, 2, false);

    let db = OregonDb::open(dir.path()).unwrap();
    let mut record = db.get_index(ids[2]).unwrap().unwrap();
    let mut body = db.get_block(ids[2]).unwrap().unwrap();
    let undo = db.get_undo(ids[2]).unwrap().unwrap();
    record.header.previous_block = config.anchor_header.block_id();
    record.parent = config.anchor_header.block_id();
    let tampered_id = record.header.block_id();
    body.header = record.header.clone();

    let mut batch = StorageBatch::new();
    batch.put_index(record);
    batch.put_block(body);
    batch.put_undo(tampered_id, undo);
    batch.set_active_height(2, tampered_id);
    batch.set_tip(tampered_id, 2);
    db.commit_durable(batch).unwrap();
    drop(db);

    assert!(ChainState::open(dir.path(), config).is_err());
}

#[test]
fn lower_work_candidate_stays_side_chain_without_mutating_active_state() {
    let dir = TestDir::new("lower-work-side-chain");
    let config = test_config();
    let ids = seed_storage_chain(dir.path(), &config, 2, false);
    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();
    let before_tip = state.tip().clone();
    let before_utxos = sorted_utxos(&state);

    let candidate_transactions = vec![coinbase(&config, 1)];
    let candidate = Block {
        header: BlockHeader {
            version: 1,
            previous_block: config.anchor_header.block_id(),
            transaction_root: transaction_root(&candidate_transactions).unwrap(),
            timestamp: config.genesis_timestamp + 300,
            difficulty_commitment: config.params.initial_target.to_le_bytes(),
            nonce: 303,
        },
        transactions: candidate_transactions,
    };
    let candidate_id = candidate.header.block_id();
    assert_eq!(
        state.accept_block(candidate, &AcceptAllSpends).unwrap(),
        AcceptOutcome::StoredSideChain
    );
    assert_eq!(state.tip(), &before_tip);
    assert_eq!(sorted_utxos(&state), before_utxos);
    drop(state);

    let db = OregonDb::open(dir.path()).unwrap();
    let index = db.get_index(candidate_id).unwrap().unwrap();
    assert_eq!(index.validation, ValidationStatus::HeaderValidated);
    assert_eq!(index.height, 1);
    assert_eq!(db.active_tip().unwrap(), Some((ids[2], 2)));
}
