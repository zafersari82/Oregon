use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rocksdb::{ColumnFamilyDescriptor, DB, Options};

use crate::db::{CF_BLOCK_INDEX, CF_BLOCKS, CF_CHAIN_META, CF_UNDO, CF_UTXO, OregonDb};
use crate::error::StorageError;
use crate::records::SCHEMA_MIGRATION_KEY;
use crate::schema::SchemaVersion;

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

fn open_raw_existing(path: &Path) -> DB {
    let options = Options::default();
    let descriptors = [CF_BLOCKS, CF_BLOCK_INDEX, CF_UTXO, CF_UNDO, CF_CHAIN_META]
        .into_iter()
        .map(|name| ColumnFamilyDescriptor::new(name, Options::default()));
    DB::open_cf_descriptors(&options, path, descriptors).unwrap()
}

#[test]
fn synthetic_minor_migration_resumes_after_reopen_and_clears_marker() {
    let dir = TestDir::new("migration-resume");
    drop(OregonDb::open(dir.path()).unwrap());

    let interrupted = OregonDb::open_with_synthetic_migration_1_1(dir.path(), true);
    assert!(matches!(
        interrupted,
        Err(StorageError::DurabilityFailure(message)) if message.contains("injected migration interruption")
    ));

    let raw = open_raw_existing(dir.path());
    let meta = raw.cf_handle(CF_CHAIN_META).unwrap();
    assert!(raw.get_cf(meta, SCHEMA_MIGRATION_KEY).unwrap().is_some());
    assert_eq!(
        raw.get_cf(meta, b"test/migration/step1")
            .unwrap()
            .as_deref(),
        Some(b"applied".as_slice())
    );
    assert!(raw.get_cf(meta, b"test/migration/step2").unwrap().is_none());
    drop(raw);

    let resumed = OregonDb::open_with_synthetic_migration_1_1(dir.path(), false).unwrap();
    assert_eq!(
        resumed.schema_version().unwrap(),
        SchemaVersion { major: 1, minor: 1 }
    );
    drop(resumed);

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
