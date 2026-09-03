use std::path::Path;

use rocksdb::{
    ColumnFamilyDescriptor, DB, DEFAULT_COLUMN_FAMILY_NAME, IteratorMode, Options, WriteOptions,
};

use crate::error::StorageError;
use crate::schema::{
    SCHEMA_KEY, SCHEMA_VERSION, SchemaVersion, decode_schema_version, encode_schema_version,
};

pub const CF_BLOCKS: &str = "blocks";
pub const CF_BLOCK_INDEX: &str = "block_index";
pub const CF_UTXO: &str = "utxo";
pub const CF_UNDO: &str = "undo";
pub const CF_CHAIN_META: &str = "chain_meta";

const OREGON_COLUMN_FAMILIES: [&str; 5] =
    [CF_BLOCKS, CF_BLOCK_INDEX, CF_UTXO, CF_UNDO, CF_CHAIN_META];

pub struct OregonDb {
    db: DB,
}

impl OregonDb {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);

        let descriptors = OREGON_COLUMN_FAMILIES
            .into_iter()
            .map(|name| ColumnFamilyDescriptor::new(name, Options::default()));
        let db = DB::open_cf_descriptors(&options, path, descriptors)?;

        let chain_meta = db.cf_handle(CF_CHAIN_META).ok_or_else(|| {
            StorageError::CorruptData("missing chain_meta column family".to_owned())
        })?;

        match db.get_cf(chain_meta, SCHEMA_KEY)? {
            Some(bytes) => {
                let version = decode_schema_version(&bytes)?;
                if version != SCHEMA_VERSION {
                    return Err(StorageError::UnsupportedSchema(version));
                }
            }
            None => {
                if !database_has_no_user_records(&db)? {
                    return Err(StorageError::CorruptData(
                        "missing schema version in non-empty database".to_owned(),
                    ));
                }

                let mut write_options = WriteOptions::default();
                write_options.set_sync(true);
                write_options.disable_wal(false);
                db.put_cf_opt(
                    chain_meta,
                    SCHEMA_KEY,
                    encode_schema_version(SCHEMA_VERSION),
                    &write_options,
                )
                .map_err(|error| StorageError::DurabilityFailure(error.to_string()))?;
            }
        }

        Ok(Self { db })
    }

    pub fn schema_version(&self) -> Result<SchemaVersion, StorageError> {
        let chain_meta = self.db.cf_handle(CF_CHAIN_META).ok_or_else(|| {
            StorageError::CorruptData("missing chain_meta column family".to_owned())
        })?;
        let bytes = self
            .db
            .get_cf(chain_meta, SCHEMA_KEY)?
            .ok_or_else(|| StorageError::CorruptData("missing schema version".to_owned()))?;
        decode_schema_version(&bytes)
    }

    #[cfg(test)]
    pub(crate) fn has_column_family(&self, name: &str) -> bool {
        self.db.cf_handle(name).is_some()
    }
}

fn database_has_no_user_records(db: &DB) -> Result<bool, StorageError> {
    for name in OREGON_COLUMN_FAMILIES
        .into_iter()
        .chain(std::iter::once(DEFAULT_COLUMN_FAMILY_NAME))
    {
        let column_family = db
            .cf_handle(name)
            .ok_or_else(|| StorageError::CorruptData(format!("missing {name} column family")))?;
        match db.iterator_cf(column_family, IteratorMode::Start).next() {
            None => {}
            Some(Ok(_)) => return Ok(false),
            Some(Err(error)) => return Err(StorageError::RocksDb(error)),
        }
    }
    Ok(true)
}
