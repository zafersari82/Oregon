use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use oregon_consensus::ChainWork;
use oregon_primitives::{Amount, BlockHeader, Hash256, OutPoint, TxOutput};
use oregon_utxo::{BlockUndo, UtxoEntry};
use rocksdb::{ColumnFamilyDescriptor, DB, Options};

use crate::{
    BlockIndexRecord, CF_BLOCK_INDEX, CF_BLOCKS, CF_CHAIN_META, CF_UNDO, CF_UTXO, NodeHealth,
    OregonDb, SchemaVersion, StorageError, ValidationStatus, active_height_key, decode_block_index,
    decode_block_undo, decode_node_health, decode_outpoint_key, decode_utxo_entry,
    encode_block_index, encode_block_undo, encode_node_health, encode_outpoint_key,
    encode_utxo_entry,
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

fn open_raw_without_schema(path: &Path) -> DB {
    let mut options = Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    let descriptors = [CF_BLOCKS, CF_BLOCK_INDEX, CF_UTXO, CF_UNDO, CF_CHAIN_META]
        .into_iter()
        .map(|name| ColumnFamilyDescriptor::new(name, Options::default()));
    DB::open_cf_descriptors(&options, path, descriptors).unwrap()
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

fn sample_sorted_undo() -> BlockUndo {
    let first = OutPoint {
        txid: Hash256::from_bytes([0x11; 32]),
        index: 0,
    };
    let second = OutPoint {
        txid: Hash256::from_bytes([0x22; 32]),
        index: 1,
    };
    BlockUndo {
        spent: vec![(first, sample_utxo(100))],
        created: vec![second],
    }
}

fn sample_header() -> BlockHeader {
    BlockHeader {
        version: 1,
        previous_block: Hash256::from_bytes([0x10; 32]),
        transaction_root: Hash256::from_bytes([0x20; 32]),
        timestamp: 1_800_000_300,
        difficulty_commitment: [0xff; 32],
        nonce: 7,
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

#[test]
fn new_database_opens_with_schema_1_0_and_required_column_families() {
    let dir = TestDir::new("schema-open");
    let db = OregonDb::open(dir.path()).unwrap();
    assert_eq!(
        db.schema_version().unwrap(),
        SchemaVersion { major: 1, minor: 0 }
    );
    for name in [CF_BLOCKS, CF_BLOCK_INDEX, CF_UTXO, CF_UNDO, CF_CHAIN_META] {
        assert!(db.has_column_family(name));
    }
    drop(db);

    let reopened = OregonDb::open(dir.path()).unwrap();
    assert_eq!(
        reopened.schema_version().unwrap(),
        SchemaVersion { major: 1, minor: 0 }
    );
}

#[test]
fn schema_less_empty_database_can_finish_initialization() {
    let dir = TestDir::new("schema-empty");
    drop(open_raw_without_schema(dir.path()));

    let db = OregonDb::open(dir.path()).unwrap();
    assert_eq!(
        db.schema_version().unwrap(),
        SchemaVersion { major: 1, minor: 0 }
    );
}

#[test]
fn schema_less_database_with_existing_data_fails_closed() {
    let dir = TestDir::new("schema-populated");
    let raw = open_raw_without_schema(dir.path());
    let blocks = raw.cf_handle(CF_BLOCKS).unwrap();
    raw.put_cf(blocks, b"orphan/block", b"non-empty").unwrap();
    drop(raw);

    let result = OregonDb::open(dir.path());
    assert!(matches!(result, Err(StorageError::CorruptData(_))));
}

#[test]
fn outpoint_key_is_exactly_36_bytes_and_little_endian_indexed() {
    let point = OutPoint {
        txid: Hash256::from_bytes([0x11; 32]),
        index: 0x0102_0304,
    };
    let key = encode_outpoint_key(&point);
    assert_eq!(&key[..32], &[0x11; 32]);
    assert_eq!(&key[32..], &[0x04, 0x03, 0x02, 0x01]);
    assert_eq!(decode_outpoint_key(&key).unwrap(), point);
    assert!(matches!(
        decode_outpoint_key(&key[..35]),
        Err(StorageError::CorruptData(_))
    ));
}

#[test]
fn utxo_codec_is_bounded_versioned_and_exact() {
    let mut max = sample_utxo(100);
    max.output.locking_program = vec![0x51; 65_536];
    let encoded = encode_utxo_entry(&max).unwrap();
    assert_eq!(decode_utxo_entry(&encoded).unwrap(), max);

    let mut too_large = sample_utxo(100);
    too_large.output.locking_program = vec![0x51; 65_537];
    assert!(matches!(
        encode_utxo_entry(&too_large),
        Err(StorageError::CorruptData(_))
    ));

    let mut unknown_version = encoded.clone();
    unknown_version[0] = 2;
    assert!(matches!(
        decode_utxo_entry(&unknown_version),
        Err(StorageError::CorruptData(_))
    ));

    assert!(matches!(
        decode_utxo_entry(&encoded[..encoded.len() - 1]),
        Err(StorageError::CorruptData(_))
    ));
    let mut trailing = encoded;
    trailing.push(0);
    assert!(matches!(
        decode_utxo_entry(&trailing),
        Err(StorageError::CorruptData(_))
    ));
}

#[test]
fn block_undo_encoding_is_deterministic_and_strictly_sorted() {
    let undo = sample_sorted_undo();
    let first = encode_block_undo(&undo).unwrap();
    assert_eq!(encode_block_undo(&undo).unwrap(), first);
    assert_eq!(decode_block_undo(&first).unwrap(), undo);

    let mut tainted = first;
    tainted.push(0);
    assert!(matches!(
        decode_block_undo(&tainted),
        Err(StorageError::CorruptData(_))
    ));

    let first_point = OutPoint {
        txid: Hash256::from_bytes([0x11; 32]),
        index: 0,
    };
    let second_point = OutPoint {
        txid: Hash256::from_bytes([0x22; 32]),
        index: 0,
    };
    let duplicate = BlockUndo {
        spent: vec![(first_point, sample_utxo(1)), (first_point, sample_utxo(2))],
        created: vec![],
    };
    assert!(matches!(
        encode_block_undo(&duplicate),
        Err(StorageError::CorruptData(_))
    ));

    let unsorted = BlockUndo {
        spent: vec![
            (second_point, sample_utxo(1)),
            (first_point, sample_utxo(2)),
        ],
        created: vec![],
    };
    assert!(matches!(
        encode_block_undo(&unsorted),
        Err(StorageError::CorruptData(_))
    ));
}

#[test]
fn block_undo_order_is_semantic_even_when_little_endian_key_bytes_disagree() {
    let txid = Hash256::from_bytes([0x33; 32]);
    let lower = OutPoint { txid, index: 1 };
    let higher = OutPoint { txid, index: 256 };
    assert!(lower < higher);
    assert!(encode_outpoint_key(&lower) > encode_outpoint_key(&higher));

    let undo = BlockUndo {
        spent: vec![(lower, sample_utxo(1)), (higher, sample_utxo(2))],
        created: vec![],
    };
    let encoded = encode_block_undo(&undo).unwrap();
    assert_eq!(decode_block_undo(&encoded).unwrap(), undo);
}

#[test]
fn block_index_codec_binds_parent_and_canonical_chainwork() {
    let index = sample_index();
    let encoded = encode_block_index(&index).unwrap();
    assert_eq!(decode_block_index(&encoded).unwrap(), index);

    let mut wrong_parent = index.clone();
    wrong_parent.parent = Hash256::from_bytes([0x99; 32]);
    assert!(matches!(
        encode_block_index(&wrong_parent),
        Err(StorageError::CorruptData(_))
    ));

    let mut non_minimal_work = encoded;
    const CHAINWORK_LEN_OFFSET: usize = 1 + 114 + 32 + 8;
    assert_eq!(non_minimal_work[CHAINWORK_LEN_OFFSET], 1);
    non_minimal_work[CHAINWORK_LEN_OFFSET] = 2;
    non_minimal_work.insert(CHAINWORK_LEN_OFFSET + 1, 0);
    assert!(matches!(
        decode_block_index(&non_minimal_work),
        Err(StorageError::CorruptData(_))
    ));
}

#[test]
fn node_health_codec_is_versioned_and_exact() {
    for health in [NodeHealth::Healthy, NodeHealth::ReindexRequired] {
        let encoded = encode_node_health(health);
        assert_eq!(decode_node_health(&encoded).unwrap(), health);
    }
    assert!(matches!(
        decode_node_health(&[2, 0]),
        Err(StorageError::CorruptData(_))
    ));
    assert!(matches!(
        decode_node_health(&[1, 2]),
        Err(StorageError::CorruptData(_))
    ));
    assert!(matches!(
        decode_node_health(&[1, 0, 0]),
        Err(StorageError::CorruptData(_))
    ));
}

#[test]
fn active_height_key_is_big_endian_and_lexicographically_ordered() {
    let key = active_height_key(0x0102_0304_0506_0708);
    assert_eq!(&key[..7], b"active/");
    assert_eq!(&key[7..], &[1, 2, 3, 4, 5, 6, 7, 8]);
    assert!(active_height_key(1) < active_height_key(2));
}
