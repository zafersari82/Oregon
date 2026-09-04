use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use oregon_consensus::ChainWork;
use oregon_primitives::{Amount, BlockHeader, Hash256, OutPoint, TxOutput};
use oregon_utxo::{BlockUndo, UtxoEntry};
use rocksdb::{ColumnFamilyDescriptor, DB, IteratorMode, Options};

use crate::batch::StorageBatch;
use crate::db::{CF_BLOCK_INDEX, CF_BLOCKS, CF_CHAIN_META, CF_UNDO, CF_UTXO, OregonDb};
use crate::error::StorageError;
use crate::records::{
    BlockIndexRecord, SCHEMA_MIGRATION_KEY, ValidationStatus, encode_block_index,
};
use crate::schema::SchemaVersion;

struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "oregon-recovery-{label}-{}-{n}",
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

fn open_raw_existing(path: &Path) -> DB {
    let options = Options::default();
    let descriptors = [CF_BLOCKS, CF_BLOCK_INDEX, CF_UTXO, CF_UNDO, CF_CHAIN_META]
        .into_iter()
        .map(|name| ColumnFamilyDescriptor::new(name, Options::default()));
    DB::open_cf_descriptors(&options, path, descriptors).unwrap()
}

fn sample_header() -> BlockHeader {
    BlockHeader {
        version: 1,
        previous_block: Hash256::from_bytes([0x10; 32]),
        transaction_root: Hash256::from_bytes([0x20; 32]),
        timestamp: 1_800_000_300,
        difficulty_commitment: [0xff; 32],
        nonce: 17,
    }
}

fn sample_index() -> BlockIndexRecord {
    let header = sample_header();
    BlockIndexRecord {
        parent: header.previous_block,
        header,
        height: 1,
        cumulative_work: ChainWork::from_canonical_be_bytes(&[1]).unwrap(),
        validation: ValidationStatus::FullyValidated,
        body_retained: true,
    }
}

fn sample_utxo(value: u64, creation_height: u64) -> UtxoEntry {
    UtxoEntry {
        output: TxOutput {
            value: Amount::from_base_units(value).unwrap(),
            locking_program: vec![0x51, value as u8],
        },
        creation_height,
        is_coinbase: false,
    }
}

fn raw_cf_entries(path: &Path, name: &str) -> Vec<(Vec<u8>, Vec<u8>)> {
    let db = open_raw_existing(path);
    let cf = db.cf_handle(name).unwrap();
    db.iterator_cf(cf, IteratorMode::Start)
        .map(|item| {
            let (key, value) = item.unwrap();
            (key.to_vec(), value.to_vec())
        })
        .collect()
}

#[test]
fn block_index_header_key_mismatch_fails_closed_on_read() {
    let dir = TestDir::new("index-key-mismatch");
    drop(OregonDb::open(dir.path()).unwrap());

    let record = sample_index();
    let encoded = encode_block_index(&record).unwrap();
    let wrong_key = Hash256::from_bytes([0x99; 32]);
    assert_ne!(wrong_key, record.header.block_id());

    let raw = open_raw_existing(dir.path());
    let index_cf = raw.cf_handle(CF_BLOCK_INDEX).unwrap();
    raw.put_cf(index_cf, wrong_key.as_bytes(), encoded).unwrap();
    drop(raw);

    let db = OregonDb::open(dir.path()).unwrap();
    assert!(matches!(
        db.get_index(wrong_key),
        Err(StorageError::CorruptData(message))
            if message.contains("key does not match decoded header block id")
    ));
}

#[test]
fn block_index_parent_header_mismatch_fails_closed_on_read() {
    let dir = TestDir::new("index-parent-mismatch");
    drop(OregonDb::open(dir.path()).unwrap());

    let record = sample_index();
    let block_id = record.header.block_id();
    let mut encoded = encode_block_index(&record).unwrap();
    const PARENT_OFFSET: usize = 1 + 114;
    encoded[PARENT_OFFSET] ^= 0x01;

    let raw = open_raw_existing(dir.path());
    let index_cf = raw.cf_handle(CF_BLOCK_INDEX).unwrap();
    raw.put_cf(index_cf, block_id.as_bytes(), encoded).unwrap();
    drop(raw);

    let db = OregonDb::open(dir.path()).unwrap();
    assert!(matches!(
        db.get_index(block_id),
        Err(StorageError::CorruptData(message))
            if message.contains("parent does not match header previous block")
    ));
}

#[test]
fn corrupt_block_and_undo_bytes_fail_closed_on_read() {
    let dir = TestDir::new("corrupt-body-undo");
    drop(OregonDb::open(dir.path()).unwrap());
    let block_id = Hash256::from_bytes([0x44; 32]);

    let raw = open_raw_existing(dir.path());
    raw.put_cf(
        raw.cf_handle(CF_BLOCKS).unwrap(),
        block_id.as_bytes(),
        [0xff, 0x00],
    )
    .unwrap();
    raw.put_cf(
        raw.cf_handle(CF_UNDO).unwrap(),
        block_id.as_bytes(),
        [0xff, 0x00],
    )
    .unwrap();
    drop(raw);

    let db = OregonDb::open(dir.path()).unwrap();
    assert!(matches!(
        db.get_block(block_id),
        Err(StorageError::CorruptData(_))
    ));
    assert!(matches!(
        db.get_undo(block_id),
        Err(StorageError::CorruptData(_))
    ));
}

#[test]
fn opposite_storage_operation_orders_produce_identical_utxo_and_undo_bytes() {
    let left = TestDir::new("deterministic-left");
    let right = TestDir::new("deterministic-right");
    let left_db = OregonDb::open(left.path()).unwrap();
    let right_db = OregonDb::open(right.path()).unwrap();

    let first = OutPoint {
        txid: Hash256::from_bytes([0x11; 32]),
        index: 0,
    };
    let second = OutPoint {
        txid: Hash256::from_bytes([0x22; 32]),
        index: 1,
    };
    let undo_id = Hash256::from_bytes([0x33; 32]);
    let undo = BlockUndo {
        spent: vec![(first, sample_utxo(100, 7)), (second, sample_utxo(200, 8))],
        created: vec![],
    };

    let mut left_batch = StorageBatch::new();
    left_batch.put_utxo(first, sample_utxo(100, 7));
    left_batch.put_utxo(second, sample_utxo(200, 8));
    left_batch.put_undo(undo_id, undo.clone());
    left_db.commit_durable(left_batch).unwrap();

    let mut right_batch = StorageBatch::new();
    right_batch.put_undo(undo_id, undo);
    right_batch.put_utxo(second, sample_utxo(200, 8));
    right_batch.put_utxo(first, sample_utxo(100, 7));
    right_db.commit_durable(right_batch).unwrap();

    drop(left_db);
    drop(right_db);

    assert_eq!(
        raw_cf_entries(left.path(), CF_UTXO),
        raw_cf_entries(right.path(), CF_UTXO)
    );
    assert_eq!(
        raw_cf_entries(left.path(), CF_UNDO),
        raw_cf_entries(right.path(), CF_UNDO)
    );
}

#[test]
fn synthetic_minor_migration_converges_when_restart_happens_before_marker() {
    let dir = TestDir::new("migration-before-marker");
    drop(OregonDb::open(dir.path()).unwrap());

    let raw = open_raw_existing(dir.path());
    let meta = raw.cf_handle(CF_CHAIN_META).unwrap();
    assert!(raw.get_cf(meta, SCHEMA_MIGRATION_KEY).unwrap().is_none());
    assert!(raw.get_cf(meta, b"test/migration/step1").unwrap().is_none());
    drop(raw);

    let migrated = OregonDb::open_with_synthetic_migration_1_1(dir.path(), false).unwrap();
    assert_eq!(
        migrated.schema_version().unwrap(),
        SchemaVersion { major: 1, minor: 1 }
    );
}

#[test]
fn synthetic_minor_migration_converges_when_restart_happens_after_marker() {
    let dir = TestDir::new("migration-after-marker");
    drop(OregonDb::open(dir.path()).unwrap());

    // marker v1: from schema 1.0 to synthetic schema 1.1
    let expected_marker = [1, 0, 1, 0, 0, 0, 1, 0, 1];
    let raw = open_raw_existing(dir.path());
    let meta = raw.cf_handle(CF_CHAIN_META).unwrap();
    raw.put_cf(meta, SCHEMA_MIGRATION_KEY, expected_marker)
        .unwrap();
    assert!(raw.get_cf(meta, b"test/migration/step1").unwrap().is_none());
    drop(raw);

    let migrated = OregonDb::open_with_synthetic_migration_1_1(dir.path(), false).unwrap();
    assert_eq!(
        migrated.schema_version().unwrap(),
        SchemaVersion { major: 1, minor: 1 }
    );
    drop(migrated);

    let raw = open_raw_existing(dir.path());
    let meta = raw.cf_handle(CF_CHAIN_META).unwrap();
    assert!(raw.get_cf(meta, SCHEMA_MIGRATION_KEY).unwrap().is_none());
    assert_eq!(
        raw.get_cf(meta, b"test/migration/step1")
            .unwrap()
            .as_deref(),
        Some(b"applied".as_slice())
    );
    assert_eq!(
        raw.get_cf(meta, b"test/migration/step2")
            .unwrap()
            .as_deref(),
        Some(b"applied".as_slice())
    );
}
