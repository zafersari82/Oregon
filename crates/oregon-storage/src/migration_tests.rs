use std::path::Path;

use oregon_primitives::Hash256;

use crate::db::{CF_CHAIN_META, OregonDb};
use crate::error::StorageError;
use crate::records::SCHEMA_MIGRATION_KEY;
use crate::schema::SchemaVersion;
use crate::test_support::{TestDir, open_raw_existing};
use crate::StorageBatch;

const LEGACY_1_0: [u8; 4] = [0, 1, 0, 0];
const TARGET_1_1: SchemaVersion = SchemaVersion { major: 1, minor: 1 };
const PREFERRED_HEADER_TIP_ID_KEY: &[u8] = b"headers/tip_id";
const PREFERRED_HEADER_TIP_HEIGHT_KEY: &[u8] = b"headers/tip_height";
const MIGRATION_1_0_TO_1_1_MARKER: [u8; 9] = [1, 0, 1, 0, 0, 0, 1, 0, 1];

fn rewrite_as_legacy_1_0(path: &Path) {
    let raw = open_raw_existing(path);
    let meta = raw.cf_handle(CF_CHAIN_META).unwrap();
    raw.put_cf(meta, b"schema/version", LEGACY_1_0).unwrap();
    raw.delete_cf(meta, SCHEMA_MIGRATION_KEY).unwrap();
    raw.delete_cf(meta, PREFERRED_HEADER_TIP_ID_KEY).unwrap();
    raw.delete_cf(meta, PREFERRED_HEADER_TIP_HEIGHT_KEY)
        .unwrap();
}

fn raw_preferred_tip(path: &Path) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let raw = open_raw_existing(path);
    let meta = raw.cf_handle(CF_CHAIN_META).unwrap();
    (
        raw.get_cf(meta, PREFERRED_HEADER_TIP_ID_KEY).unwrap(),
        raw.get_cf(meta, PREFERRED_HEADER_TIP_HEIGHT_KEY).unwrap(),
    )
}

#[test]
fn fresh_database_uses_schema_1_1() {
    let dir = TestDir::new("migration-fresh-1-1");
    let db = OregonDb::open(dir.path()).unwrap();
    assert_eq!(db.schema_version().unwrap(), TARGET_1_1);
}

#[test]
fn legacy_1_0_active_tip_migrates_to_preferred_header_tip() {
    let dir = TestDir::new("migration-active-tip");
    let block_id = Hash256::from_bytes([0x42; 32]);
    let height = 73u64;

    let db = OregonDb::open(dir.path()).unwrap();
    let mut batch = StorageBatch::new();
    batch.set_tip(block_id, height);
    db.commit_durable(batch).unwrap();
    drop(db);
    rewrite_as_legacy_1_0(dir.path());

    let migrated = OregonDb::open(dir.path()).unwrap();
    assert_eq!(migrated.schema_version().unwrap(), TARGET_1_1);
    drop(migrated);

    let (preferred_id, preferred_height) = raw_preferred_tip(dir.path());
    assert_eq!(preferred_id.as_deref(), Some(block_id.as_bytes().as_slice()));
    assert_eq!(
        preferred_height.as_deref(),
        Some(height.to_le_bytes().as_slice())
    );
}

#[test]
fn legacy_1_0_without_active_tip_migrates_without_inventing_preferred_tip() {
    let dir = TestDir::new("migration-empty-tip");
    drop(OregonDb::open(dir.path()).unwrap());
    rewrite_as_legacy_1_0(dir.path());

    let migrated = OregonDb::open(dir.path()).unwrap();
    assert_eq!(migrated.schema_version().unwrap(), TARGET_1_1);
    drop(migrated);

    assert_eq!(raw_preferred_tip(dir.path()), (None, None));
}

#[test]
fn interrupted_1_0_to_1_1_marker_resumes_idempotently() {
    let dir = TestDir::new("migration-resume-real");
    let block_id = Hash256::from_bytes([0x57; 32]);
    let height = 19u64;

    let db = OregonDb::open(dir.path()).unwrap();
    let mut batch = StorageBatch::new();
    batch.set_tip(block_id, height);
    db.commit_durable(batch).unwrap();
    drop(db);
    rewrite_as_legacy_1_0(dir.path());

    let raw = open_raw_existing(dir.path());
    let meta = raw.cf_handle(CF_CHAIN_META).unwrap();
    raw.put_cf(meta, SCHEMA_MIGRATION_KEY, MIGRATION_1_0_TO_1_1_MARKER)
        .unwrap();
    drop(raw);

    let resumed = OregonDb::open(dir.path()).unwrap();
    assert_eq!(resumed.schema_version().unwrap(), TARGET_1_1);
    drop(resumed);

    let raw = open_raw_existing(dir.path());
    let meta = raw.cf_handle(CF_CHAIN_META).unwrap();
    assert!(raw.get_cf(meta, SCHEMA_MIGRATION_KEY).unwrap().is_none());
    assert_eq!(
        raw.get_cf(meta, PREFERRED_HEADER_TIP_ID_KEY)
            .unwrap()
            .as_deref(),
        Some(block_id.as_bytes().as_slice())
    );
    assert_eq!(
        raw.get_cf(meta, PREFERRED_HEADER_TIP_HEIGHT_KEY)
            .unwrap()
            .as_deref(),
        Some(height.to_le_bytes().as_slice())
    );
}

#[test]
fn partial_preferred_header_tip_metadata_is_corrupt() {
    let dir = TestDir::new("migration-partial-preferred-tip");
    drop(OregonDb::open(dir.path()).unwrap());

    let raw = open_raw_existing(dir.path());
    let meta = raw.cf_handle(CF_CHAIN_META).unwrap();
    raw.put_cf(meta, PREFERRED_HEADER_TIP_ID_KEY, [0x66; 32])
        .unwrap();
    drop(raw);

    assert!(matches!(
        OregonDb::open(dir.path()),
        Err(StorageError::CorruptData(message)) if message.contains("preferred header tip")
    ));
}

#[test]
fn preferred_header_storage_api_is_declared_as_one_logical_batch_operation() {
    let batch_source = include_str!("batch.rs");
    let db_source = include_str!("db.rs");
    let records_source = include_str!("records.rs");

    assert!(batch_source.contains("set_preferred_header_tip"));
    assert!(batch_source.contains("SetPreferredHeaderTip"));
    assert!(db_source.contains("pub fn preferred_header_tip"));
    assert!(records_source.contains("PREFERRED_HEADER_TIP_ID_KEY"));
    assert!(records_source.contains("PREFERRED_HEADER_TIP_HEIGHT_KEY"));
}

#[test]
fn unknown_major_schema_is_rejected_without_rewrite() {
    let dir = TestDir::new("migration-major");
    drop(OregonDb::open(dir.path()).unwrap());

    let raw = open_raw_existing(dir.path());
    let meta = raw.cf_handle(CF_CHAIN_META).unwrap();
    raw.put_cf(meta, b"schema/version", [0, 2, 0, 0]).unwrap();
    drop(raw);

    assert!(matches!(
        OregonDb::open(dir.path()),
        Err(StorageError::UnsupportedSchema(SchemaVersion {
            major: 2,
            minor: 0
        }))
    ));

    let raw = open_raw_existing(dir.path());
    let meta = raw.cf_handle(CF_CHAIN_META).unwrap();
    assert_eq!(
        raw.get_cf(meta, b"schema/version").unwrap().as_deref(),
        Some([0, 2, 0, 0].as_slice())
    );
    assert!(raw.get_cf(meta, SCHEMA_MIGRATION_KEY).unwrap().is_none());
}

#[test]
fn unknown_minor_schema_is_rejected_without_guessing() {
    let dir = TestDir::new("migration-minor");
    drop(OregonDb::open(dir.path()).unwrap());

    let raw = open_raw_existing(dir.path());
    let meta = raw.cf_handle(CF_CHAIN_META).unwrap();
    raw.put_cf(meta, b"schema/version", [0, 1, 0, 2]).unwrap();
    drop(raw);

    assert!(matches!(
        OregonDb::open(dir.path()),
        Err(StorageError::UnsupportedSchema(SchemaVersion {
            major: 1,
            minor: 2
        }))
    ));
}
