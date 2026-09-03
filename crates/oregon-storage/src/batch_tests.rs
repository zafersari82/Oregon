use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use oregon_consensus::ChainWork;
use oregon_primitives::{
    Amount, Block, BlockHeader, Hash256, OutPoint, Transaction, TxOutput, transaction_root,
};
use oregon_utxo::{BlockUndo, UtxoEntry};
use rocksdb::{ColumnFamilyDescriptor, DB, Options};

use crate::{
    BlockIndexRecord, CF_BLOCK_INDEX, CF_BLOCKS, CF_CHAIN_META, CF_UNDO, CF_UTXO, DurabilityMode,
    NodeHealth, OregonDb, StorageBatch, StorageError, ValidationStatus, encode_block_index,
};

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("oregon-{label}-{}-{n}", std::process::id()));
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

fn sample_utxo(value: u64) -> UtxoEntry {
    UtxoEntry {
        output: TxOutput {
            value: Amount::from_base_units(value).unwrap(),
            locking_program: vec![0x51],
        },
        creation_height: 7,
        is_coinbase: false,
    }
}

fn sample_undo() -> BlockUndo {
    BlockUndo {
        spent: vec![(
            OutPoint {
                txid: Hash256::from_bytes([0x11; 32]),
                index: 0,
            },
            sample_utxo(100),
        )],
        created: vec![OutPoint {
            txid: Hash256::from_bytes([0x22; 32]),
            index: 1,
        }],
    }
}

fn sample_block(nonce: u64) -> Block {
    let transactions = vec![Transaction {
        version: 1,
        inputs: vec![],
        outputs: vec![TxOutput {
            value: Amount::from_base_units(50).unwrap(),
            locking_program: vec![0x51],
        }],
        lock_time: 0,
    }];
    Block {
        header: BlockHeader {
            version: 1,
            previous_block: Hash256::from_bytes([0x10; 32]),
            transaction_root: transaction_root(&transactions).unwrap(),
            timestamp: 1_800_000_300,
            difficulty_commitment: [0xff; 32],
            nonce,
        },
        transactions,
    }
}

fn sample_index(block: &Block) -> BlockIndexRecord {
    BlockIndexRecord {
        header: block.header.clone(),
        parent: block.header.previous_block,
        height: 1,
        cumulative_work: ChainWork::from_canonical_be_bytes(&[1]).unwrap(),
        validation: ValidationStatus::FullyValidated,
        body_retained: true,
    }
}

fn open_raw_existing(path: &Path) -> DB {
    let options = Options::default();
    let descriptors = [CF_BLOCKS, CF_BLOCK_INDEX, CF_UTXO, CF_UNDO, CF_CHAIN_META]
        .into_iter()
        .map(|name| ColumnFamilyDescriptor::new(name, Options::default()));
    DB::open_cf_descriptors(&options, path, descriptors).unwrap()
}

#[test]
fn durable_batch_round_trips_all_typed_state_after_reopen() {
    let dir = TestDir::new("typed-roundtrip");
    let db = OregonDb::open(dir.path()).unwrap();
    let block = sample_block(7);
    let block_id = block.header.block_id();
    let index = sample_index(&block);
    let point = OutPoint {
        txid: Hash256::from_bytes([0x33; 32]),
        index: 2,
    };
    let utxo = sample_utxo(75);
    let undo = sample_undo();

    let mut batch = StorageBatch::new();
    batch.put_block(block.clone());
    batch.put_index(index.clone());
    batch.put_utxo(point, utxo.clone());
    batch.put_undo(block_id, undo.clone());
    batch.set_active_height(1, block_id);
    batch.set_tip(block_id, 1);
    batch.set_health(NodeHealth::Healthy);
    batch.set_prune_cursor(1);
    db.commit_durable(batch).unwrap();
    drop(db);

    let reopened = OregonDb::open(dir.path()).unwrap();
    assert_eq!(reopened.get_block(block_id).unwrap(), Some(block));
    assert_eq!(reopened.get_index(block_id).unwrap(), Some(index.clone()));
    assert_eq!(reopened.get_utxo(point).unwrap(), Some(utxo.clone()));
    assert_eq!(reopened.iter_utxos().unwrap(), vec![(point, utxo)]);
    assert_eq!(reopened.get_undo(block_id).unwrap(), Some(undo));
    assert_eq!(reopened.active_id_at_height(1).unwrap(), Some(block_id));
    assert_eq!(reopened.active_tip().unwrap(), Some((block_id, 1)));
    assert_eq!(reopened.health().unwrap(), Some(NodeHealth::Healthy));
    assert_eq!(
        reopened.iter_body_retained_indices().unwrap(),
        vec![(block_id, index)]
    );
}

#[test]
fn durable_commit_requests_sync_and_maintenance_does_not() {
    let dir = TestDir::new("durability-mode");
    let db = OregonDb::open_with_test_hooks(dir.path()).unwrap();

    db.commit_durable(StorageBatch::new()).unwrap();
    assert_eq!(db.test_hooks().last_mode(), Some(DurabilityMode::Sync));

    db.commit_maintenance(StorageBatch::new()).unwrap();
    assert_eq!(
        db.test_hooks().last_mode(),
        Some(DurabilityMode::NoSync)
    );
}

#[test]
fn injected_durable_failure_happens_before_any_rocksdb_mutation() {
    let dir = TestDir::new("durable-failure");
    let db = OregonDb::open_with_test_hooks(dir.path()).unwrap();
    let block = sample_block(8);
    let block_id = block.header.block_id();
    let mut batch = StorageBatch::new();
    batch.put_block(block);

    db.test_hooks().fail_next_durable_write();
    assert!(matches!(
        db.commit_durable(batch),
        Err(StorageError::DurabilityFailure(_))
    ));
    assert_eq!(db.get_block(block_id).unwrap(), None);
}

#[test]
fn typed_block_and_index_reads_reject_key_identity_mismatch() {
    let dir = TestDir::new("identity-check");
    let db = OregonDb::open(dir.path()).unwrap();
    let first = sample_block(9);
    let first_id = first.header.block_id();
    let mut batch = StorageBatch::new();
    batch.put_block(first.clone());
    batch.put_index(sample_index(&first));
    db.commit_durable(batch).unwrap();
    drop(db);

    let second = sample_block(10);
    let raw = open_raw_existing(dir.path());
    raw.put_cf(
        raw.cf_handle(CF_BLOCKS).unwrap(),
        first_id.as_bytes(),
        second.encode(),
    )
    .unwrap();
    raw.put_cf(
        raw.cf_handle(CF_BLOCK_INDEX).unwrap(),
        first_id.as_bytes(),
        encode_block_index(&sample_index(&second)).unwrap(),
    )
    .unwrap();
    drop(raw);

    let reopened = OregonDb::open(dir.path()).unwrap();
    assert!(matches!(
        reopened.get_block(first_id),
        Err(StorageError::CorruptData(_))
    ));
    assert!(matches!(
        reopened.get_index(first_id),
        Err(StorageError::CorruptData(_))
    ));
}
