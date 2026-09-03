use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rocksdb::{ColumnFamilyDescriptor, DB, Options};

use crate::{
    CF_BLOCK_INDEX, CF_BLOCKS, CF_CHAIN_META, CF_UNDO, CF_UTXO, OregonDb, SchemaVersion,
    StorageError,
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
