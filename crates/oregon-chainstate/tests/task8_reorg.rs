use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use oregon_chainstate::{AcceptOutcome, ChainConfig, ChainState, ChainStateError, SessionHealth};
use oregon_consensus::{ConsensusParams, Target, params::KEY_COMMIT_V1};
use oregon_primitives::{
    Amount, Block, BlockHeader, FOUNDER_ALLOCATION_BASE_UNITS, Hash256, OutPoint, Transaction,
    TxInput, TxOutput, transaction_root, write_varint,
};
use oregon_storage::{OregonDb, ValidationStatus};
use oregon_utxo::{SpendVerifier, UtxoEntry, UtxoError};

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("oregon-task8-{label}-{}-{n}", std::process::id()));
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

struct AcceptTestSpends;

impl SpendVerifier for AcceptTestSpends {
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

fn coinbase(config: &ChainConfig, height: u64, witness_height: u64, miner_tag: u8) -> Transaction {
    let mut height_bytes = Vec::new();
    write_varint(witness_height, &mut height_bytes);
    let mut outputs = Vec::new();
    if height == 1 {
        let mut founder_program = vec![KEY_COMMIT_V1];
        founder_program.extend_from_slice(&config.params.founder_key_commitment);
        outputs.push(TxOutput {
            value: Amount::from_base_units(FOUNDER_ALLOCATION_BASE_UNITS).unwrap(),
            locking_program: founder_program,
        });
    }
    outputs.push(TxOutput {
        value: Amount::from_base_units(1).unwrap(),
        locking_program: vec![miner_tag],
    });
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

fn block(
    config: &ChainConfig,
    parent: Hash256,
    height: u64,
    nonce_domain: u64,
    miner_tag: u8,
    witness_height: u64,
) -> Block {
    let transactions = vec![coinbase(config, height, witness_height, miner_tag)];
    Block {
        header: BlockHeader {
            version: 1,
            previous_block: parent,
            transaction_root: transaction_root(&transactions).unwrap(),
            timestamp: config.genesis_timestamp + height * 300,
            difficulty_commitment: config.params.initial_target.to_le_bytes(),
            nonce: nonce_domain + height,
        },
        transactions,
    }
}

fn extend_active(
    state: &mut ChainState,
    config: &ChainConfig,
    parent: Hash256,
    height: u64,
    nonce_domain: u64,
    tag: u8,
) -> Block {
    let block = block(config, parent, height, nonce_domain, tag, height);
    assert_eq!(
        state
            .accept_block(block.clone(), &AcceptTestSpends)
            .unwrap(),
        AcceptOutcome::Extended
    );
    block
}

#[test]
fn valid_reorg_is_atomic_and_reopens_on_strictly_heavier_candidate() {
    let dir = TestDir::new("valid-reorg");
    let config = config();
    let anchor = config.anchor_header.block_id();
    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();

    let active1 = extend_active(&mut state, &config, anchor, 1, 10_000, 0xa1);
    let active2 = extend_active(
        &mut state,
        &config,
        active1.header.block_id(),
        2,
        10_000,
        0xa2,
    );
    let old_tip = active2.header.block_id();
    let old_outpoint = OutPoint {
        txid: active2.transactions[0].txid(),
        index: 0,
    };
    assert!(state.utxos().get(&old_outpoint).is_some());

    let candidate1 = block(&config, anchor, 1, 20_000, 0xb1, 1);
    let candidate1_id = candidate1.header.block_id();
    assert_eq!(
        state
            .accept_block(candidate1.clone(), &AcceptTestSpends)
            .unwrap(),
        AcceptOutcome::StoredSideChain
    );
    let candidate2 = block(&config, candidate1_id, 2, 20_000, 0xb2, 2);
    let candidate2_id = candidate2.header.block_id();
    assert_eq!(
        state
            .accept_block(candidate2.clone(), &AcceptTestSpends)
            .unwrap(),
        AcceptOutcome::StoredSideChain
    );
    let candidate3 = block(&config, candidate2_id, 3, 20_000, 0xb3, 3);
    let candidate3_id = candidate3.header.block_id();
    let candidate3_outpoint = OutPoint {
        txid: candidate3.transactions[0].txid(),
        index: 0,
    };

    assert_eq!(
        state
            .accept_block(candidate3.clone(), &AcceptTestSpends)
            .unwrap(),
        AcceptOutcome::Reorganized
    );
    assert_eq!(state.tip().block_id, candidate3_id);
    assert_eq!(state.tip().height, 3);
    assert!(state.utxos().get(&old_outpoint).is_none());
    assert!(state.utxos().get(&candidate3_outpoint).is_some());
    assert_eq!(state.session_health(), SessionHealth::Healthy);
    drop(state);

    let reopened = ChainState::open(dir.path(), config.clone()).unwrap();
    assert_eq!(reopened.tip().block_id, candidate3_id);
    assert_eq!(reopened.tip().height, 3);
    assert!(reopened.utxos().get(&old_outpoint).is_none());
    assert!(reopened.utxos().get(&candidate3_outpoint).is_some());
    drop(reopened);

    let db = OregonDb::open(dir.path()).unwrap();
    assert_eq!(db.active_tip().unwrap(), Some((candidate3_id, 3)));
    assert_eq!(db.active_id_at_height(1).unwrap(), Some(candidate1_id));
    assert_eq!(db.active_id_at_height(2).unwrap(), Some(candidate2_id));
    assert_eq!(db.active_id_at_height(3).unwrap(), Some(candidate3_id));
    assert_ne!(db.active_tip().unwrap(), Some((old_tip, 2)));
    for id in [candidate1_id, candidate2_id, candidate3_id] {
        assert_eq!(
            db.get_index(id).unwrap().unwrap().validation,
            ValidationStatus::FullyValidated
        );
        assert!(db.get_undo(id).unwrap().is_some());
    }
}

#[test]
fn invalid_candidate_body_marks_failing_block_and_descendants_without_active_publication() {
    let dir = TestDir::new("invalid-candidate");
    let config = config();
    let anchor = config.anchor_header.block_id();
    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();

    let active1 = extend_active(&mut state, &config, anchor, 1, 30_000, 0xc1);
    let active2 = extend_active(
        &mut state,
        &config,
        active1.header.block_id(),
        2,
        30_000,
        0xc2,
    );
    let active3 = extend_active(
        &mut state,
        &config,
        active2.header.block_id(),
        3,
        30_000,
        0xc3,
    );
    let before_tip = state.tip().clone();
    let before_utxos = state.utxos().clone();

    let candidate1 = block(&config, anchor, 1, 40_000, 0xd1, 1);
    let candidate1_id = candidate1.header.block_id();
    assert_eq!(
        state.accept_block(candidate1, &AcceptTestSpends).unwrap(),
        AcceptOutcome::StoredSideChain
    );

    let candidate2 = block(&config, candidate1_id, 2, 40_000, 0xd2, 99);
    let candidate2_id = candidate2.header.block_id();
    assert_eq!(
        state.accept_block(candidate2, &AcceptTestSpends).unwrap(),
        AcceptOutcome::StoredSideChain
    );

    let candidate3 = block(&config, candidate2_id, 3, 40_000, 0xd3, 3);
    let candidate3_id = candidate3.header.block_id();
    assert_eq!(
        state.accept_block(candidate3, &AcceptTestSpends).unwrap(),
        AcceptOutcome::StoredSideChain
    );

    let candidate4 = block(&config, candidate3_id, 4, 40_000, 0xd4, 4);
    let candidate4_id = candidate4.header.block_id();
    assert!(matches!(
        state.accept_block(candidate4, &AcceptTestSpends),
        Err(ChainStateError::Utxo(UtxoError::Consensus(_)))
    ));
    assert_eq!(state.tip(), &before_tip);
    assert_eq!(state.utxos(), &before_utxos);
    assert_eq!(state.session_health(), SessionHealth::Healthy);
    drop(state);

    let db = OregonDb::open(dir.path()).unwrap();
    assert_eq!(
        db.active_tip().unwrap(),
        Some((active3.header.block_id(), 3))
    );
    assert_eq!(
        db.get_index(candidate1_id).unwrap().unwrap().validation,
        ValidationStatus::HeaderValidated
    );
    for id in [candidate2_id, candidate3_id, candidate4_id] {
        assert_eq!(
            db.get_index(id).unwrap().unwrap().validation,
            ValidationStatus::Invalid
        );
    }
}
