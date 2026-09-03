use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use oregon_chainstate::{AcceptOutcome, ChainConfig, ChainState};
use oregon_consensus::{ConsensusParams, Target, block_work, params::KEY_COMMIT_V1};
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
            std::env::temp_dir().join(format!("oregon-task7-{label}-{}-{n}", std::process::id()));
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

fn height_one_founder_block(config: &ChainConfig) -> Block {
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
            nonce: 101,
        },
        transactions,
    }
}

#[test]
fn direct_extension_persists_tip_undo_and_founder_utxo_across_reopen() {
    let dir = TestDir::new("direct-extension");
    let config = config();
    let block = height_one_founder_block(&config);
    let block_id = block.header.block_id();
    let founder_outpoint = OutPoint {
        txid: block.transactions[0].txid(),
        index: 0,
    };

    let mut state = ChainState::open(dir.path(), config.clone()).unwrap();
    assert_eq!(
        state
            .accept_block(block.clone(), &AcceptTestSpends)
            .unwrap(),
        AcceptOutcome::Extended
    );
    assert_eq!(state.tip().block_id, block_id);
    assert_eq!(state.tip().height, 1);
    assert_eq!(
        state.tip().cumulative_work,
        block_work(config.params.initial_target)
    );
    let founder = state.utxos().get(&founder_outpoint).unwrap();
    assert_eq!(
        founder.output.value.base_units(),
        FOUNDER_ALLOCATION_BASE_UNITS
    );
    assert_eq!(founder.creation_height, 1);
    assert!(founder.is_coinbase);
    drop(state);

    let reopened = ChainState::open(dir.path(), config.clone()).unwrap();
    assert_eq!(reopened.tip().block_id, block_id);
    assert_eq!(reopened.tip().height, 1);
    assert_eq!(
        reopened.tip().cumulative_work,
        block_work(config.params.initial_target)
    );
    let founder = reopened.utxos().get(&founder_outpoint).unwrap();
    assert_eq!(
        founder.output.value.base_units(),
        FOUNDER_ALLOCATION_BASE_UNITS
    );
    assert_eq!(founder.creation_height, 1);
    assert!(founder.is_coinbase);
    drop(reopened);

    let db = OregonDb::open(dir.path()).unwrap();
    let index = db.get_index(block_id).unwrap().unwrap();
    assert_eq!(index.validation, ValidationStatus::FullyValidated);
    assert!(index.body_retained);
    assert_eq!(db.get_block(block_id).unwrap(), Some(block));
    assert!(db.get_undo(block_id).unwrap().is_some());
    assert_eq!(db.active_id_at_height(1).unwrap(), Some(block_id));
    assert_eq!(db.active_tip().unwrap(), Some((block_id, 1)));
}
