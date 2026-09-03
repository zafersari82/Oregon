use std::path::Path;
#[cfg(any(test, feature = "test-hooks"))]
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use oregon_primitives::{Block, DecodeLimits, Hash256, OutPoint};
use oregon_utxo::{BlockUndo, UtxoEntry};
use rocksdb::{
    ColumnFamilyDescriptor, DB, DEFAULT_COLUMN_FAMILY_NAME, IteratorMode, Options, WriteBatch,
    WriteOptions,
};

use crate::batch::{DurabilityMode, StorageBatch, StorageOp};
use crate::codec::{
    decode_block_undo, decode_outpoint_key, decode_utxo_entry, encode_block_undo,
    encode_outpoint_key, encode_utxo_entry,
};
use crate::error::StorageError;
#[cfg(test)]
use crate::records::SCHEMA_MIGRATION_KEY;
use crate::records::{
    ACTIVE_TIP_HEIGHT_KEY, ACTIVE_TIP_ID_KEY, BlockIndexRecord, CONFIG_ANCHOR_ID_KEY,
    CONFIG_GENESIS_TIMESTAMP_KEY, HEALTH_STATE_KEY, NodeHealth, PRUNE_CURSOR_KEY,
    active_height_key, decode_block_index, decode_node_health, encode_block_index,
    encode_node_health,
};
use crate::schema::{
    SCHEMA_KEY, SCHEMA_VERSION, SchemaVersion, decode_schema_version, encode_schema_version,
};
#[cfg(test)]
use crate::schema::{decode_migration_marker, encode_migration_marker};

pub const CF_BLOCKS: &str = "blocks";
pub const CF_BLOCK_INDEX: &str = "block_index";
pub const CF_UTXO: &str = "utxo";
pub const CF_UNDO: &str = "undo";
pub const CF_CHAIN_META: &str = "chain_meta";

const OREGON_COLUMN_FAMILIES: [&str; 5] =
    [CF_BLOCKS, CF_BLOCK_INDEX, CF_UTXO, CF_UNDO, CF_CHAIN_META];

pub struct OregonDb {
    db: DB,
    #[cfg(any(test, feature = "test-hooks"))]
    test_hooks: TestHooks,
}

#[cfg(any(test, feature = "test-hooks"))]
#[derive(Debug, Default)]
pub struct TestHooks {
    fail_next_durable: AtomicBool,
    fail_next_maintenance: AtomicBool,
    last_mode: Mutex<Option<DurabilityMode>>,
}

#[cfg(any(test, feature = "test-hooks"))]
impl TestHooks {
    pub fn fail_next_durable_write(&self) {
        self.fail_next_durable.store(true, Ordering::SeqCst);
    }

    pub fn fail_next_maintenance_write(&self) {
        self.fail_next_maintenance.store(true, Ordering::SeqCst);
    }

    pub fn last_mode(&self) -> Option<DurabilityMode> {
        *self
            .last_mode
            .lock()
            .expect("storage test hook mutex poisoned")
    }

    fn record_mode(&self, mode: DurabilityMode) {
        *self
            .last_mode
            .lock()
            .expect("storage test hook mutex poisoned") = Some(mode);
    }

    fn should_fail(&self, mode: DurabilityMode) -> bool {
        match mode {
            DurabilityMode::Sync => self.fail_next_durable.swap(false, Ordering::SeqCst),
            DurabilityMode::NoSync => self.fail_next_maintenance.swap(false, Ordering::SeqCst),
        }
    }
}

impl OregonDb {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        Self::open_internal(path)
    }

    #[cfg(any(test, feature = "test-hooks"))]
    pub fn open_with_test_hooks(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        Self::open_internal(path)
    }

    #[cfg(test)]
    pub(crate) fn open_with_synthetic_migration_1_1(
        path: impl AsRef<Path>,
        interrupt_after_first_step: bool,
    ) -> Result<Self, StorageError> {
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
        let bytes = db
            .get_cf(chain_meta, SCHEMA_KEY)?
            .ok_or_else(|| StorageError::CorruptData("missing schema version".to_owned()))?;
        let current = decode_schema_version(&bytes)?;
        let target = SchemaVersion { major: 1, minor: 1 };

        if current.major != target.major {
            return Err(StorageError::UnsupportedSchema(current));
        }
        if current == target {
            if db.get_cf(chain_meta, SCHEMA_MIGRATION_KEY)?.is_some() {
                return Err(StorageError::CorruptData(
                    "completed synthetic migration still has a marker".to_owned(),
                ));
            }
            return Ok(Self {
                db,
                test_hooks: TestHooks::default(),
            });
        }
        if current != SCHEMA_VERSION {
            return Err(StorageError::UnsupportedSchema(current));
        }

        run_synthetic_minor_migration_1_1(&db, chain_meta, interrupt_after_first_step)?;

        Ok(Self {
            db,
            test_hooks: TestHooks::default(),
        })
    }

    fn open_internal(path: impl AsRef<Path>) -> Result<Self, StorageError> {
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

        Ok(Self {
            db,
            #[cfg(any(test, feature = "test-hooks"))]
            test_hooks: TestHooks::default(),
        })
    }

    pub fn schema_version(&self) -> Result<SchemaVersion, StorageError> {
        let chain_meta = self.column_family(CF_CHAIN_META)?;
        let bytes = self
            .db
            .get_cf(chain_meta, SCHEMA_KEY)?
            .ok_or_else(|| StorageError::CorruptData("missing schema version".to_owned()))?;
        decode_schema_version(&bytes)
    }

    pub fn commit_durable(&self, batch: StorageBatch) -> Result<(), StorageError> {
        self.commit(batch, DurabilityMode::Sync)
    }

    pub fn commit_maintenance(&self, batch: StorageBatch) -> Result<(), StorageError> {
        self.commit(batch, DurabilityMode::NoSync)
    }

    pub fn get_block(&self, block_id: Hash256) -> Result<Option<Block>, StorageError> {
        let blocks = self.column_family(CF_BLOCKS)?;
        let Some(bytes) = self.db.get_cf(blocks, block_id.as_bytes())? else {
            return Ok(None);
        };
        let block = Block::decode(&bytes, &DecodeLimits::default())
            .map_err(|error| corrupt(format!("invalid block record: {error}")))?;
        if block.header.block_id() != block_id {
            return Err(corrupt("block record key does not match decoded block id"));
        }
        Ok(Some(block))
    }

    pub fn get_index(&self, block_id: Hash256) -> Result<Option<BlockIndexRecord>, StorageError> {
        let index_cf = self.column_family(CF_BLOCK_INDEX)?;
        let Some(bytes) = self.db.get_cf(index_cf, block_id.as_bytes())? else {
            return Ok(None);
        };
        let record = decode_block_index(&bytes)?;
        if record.header.block_id() != block_id {
            return Err(corrupt(
                "block index key does not match decoded header block id",
            ));
        }
        Ok(Some(record))
    }

    pub fn get_utxo(&self, outpoint: OutPoint) -> Result<Option<UtxoEntry>, StorageError> {
        let utxo_cf = self.column_family(CF_UTXO)?;
        let key = encode_outpoint_key(&outpoint);
        self.db
            .get_cf(utxo_cf, key)?
            .map(|bytes| decode_utxo_entry(&bytes))
            .transpose()
    }

    pub fn iter_utxos(&self) -> Result<Vec<(OutPoint, UtxoEntry)>, StorageError> {
        let utxo_cf = self.column_family(CF_UTXO)?;
        let mut entries = Vec::new();
        for item in self.db.iterator_cf(utxo_cf, IteratorMode::Start) {
            let (key, value) = item?;
            entries.push((decode_outpoint_key(&key)?, decode_utxo_entry(&value)?));
        }
        Ok(entries)
    }

    pub fn get_undo(&self, block_id: Hash256) -> Result<Option<BlockUndo>, StorageError> {
        let undo_cf = self.column_family(CF_UNDO)?;
        self.db
            .get_cf(undo_cf, block_id.as_bytes())?
            .map(|bytes| decode_block_undo(&bytes))
            .transpose()
    }

    pub fn active_id_at_height(&self, height: u64) -> Result<Option<Hash256>, StorageError> {
        let meta = self.column_family(CF_CHAIN_META)?;
        let Some(bytes) = self.db.get_cf(meta, active_height_key(height))? else {
            return Ok(None);
        };
        Ok(Some(decode_hash(&bytes, "active height block id")?))
    }

    pub fn active_tip(&self) -> Result<Option<(Hash256, u64)>, StorageError> {
        let meta = self.column_family(CF_CHAIN_META)?;
        let id = self.db.get_cf(meta, ACTIVE_TIP_ID_KEY)?;
        let height = self.db.get_cf(meta, ACTIVE_TIP_HEIGHT_KEY)?;
        match (id, height) {
            (None, None) => Ok(None),
            (Some(id), Some(height)) => Ok(Some((
                decode_hash(&id, "active tip block id")?,
                decode_u64_le(&height, "active tip height")?,
            ))),
            _ => Err(corrupt("active tip metadata is partially present")),
        }
    }

    pub fn config_anchor_id(&self) -> Result<Option<Hash256>, StorageError> {
        let meta = self.column_family(CF_CHAIN_META)?;
        self.db
            .get_cf(meta, CONFIG_ANCHOR_ID_KEY)?
            .map(|bytes| decode_hash(&bytes, "config anchor id"))
            .transpose()
    }

    pub fn config_genesis_timestamp(&self) -> Result<Option<u64>, StorageError> {
        let meta = self.column_family(CF_CHAIN_META)?;
        self.db
            .get_cf(meta, CONFIG_GENESIS_TIMESTAMP_KEY)?
            .map(|bytes| decode_u64_le(&bytes, "config genesis timestamp"))
            .transpose()
    }

    pub fn health(&self) -> Result<Option<NodeHealth>, StorageError> {
        let meta = self.column_family(CF_CHAIN_META)?;
        self.db
            .get_cf(meta, HEALTH_STATE_KEY)?
            .map(|bytes| decode_node_health(&bytes))
            .transpose()
    }

    pub fn prune_cursor(&self) -> Result<Option<u64>, StorageError> {
        let meta = self.column_family(CF_CHAIN_META)?;
        self.db
            .get_cf(meta, PRUNE_CURSOR_KEY)?
            .map(|bytes| decode_u64_le(&bytes, "prune cursor"))
            .transpose()
    }

    pub fn iter_body_retained_indices(
        &self,
    ) -> Result<Vec<(Hash256, BlockIndexRecord)>, StorageError> {
        let index_cf = self.column_family(CF_BLOCK_INDEX)?;
        let mut records = Vec::new();
        for item in self.db.iterator_cf(index_cf, IteratorMode::Start) {
            let (key, value) = item?;
            let block_id = decode_hash(&key, "block index key")?;
            let record = decode_block_index(&value)?;
            if record.header.block_id() != block_id {
                return Err(corrupt(
                    "block index key does not match decoded header block id",
                ));
            }
            if record.body_retained {
                records.push((block_id, record));
            }
        }
        Ok(records)
    }

    #[cfg(any(test, feature = "test-hooks"))]
    pub fn test_hooks(&self) -> &TestHooks {
        &self.test_hooks
    }

    #[cfg(test)]
    pub(crate) fn has_column_family(&self, name: &str) -> bool {
        self.db.cf_handle(name).is_some()
    }

    fn commit(&self, batch: StorageBatch, mode: DurabilityMode) -> Result<(), StorageError> {
        let encoded = encode_operations(batch.operations)?;
        let mut write_batch = WriteBatch::default();
        for operation in encoded {
            let column_family = self.column_family(operation.column_family)?;
            match operation.value {
                Some(value) => write_batch.put_cf(column_family, operation.key, value),
                None => write_batch.delete_cf(column_family, operation.key),
            }
        }

        #[cfg(any(test, feature = "test-hooks"))]
        {
            self.test_hooks.record_mode(mode);
            if self.test_hooks.should_fail(mode) {
                return Err(StorageError::DurabilityFailure(format!(
                    "injected {mode:?} write failure"
                )));
            }
        }

        let mut write_options = WriteOptions::default();
        write_options.set_sync(matches!(mode, DurabilityMode::Sync));
        write_options.disable_wal(false);
        self.db
            .write_opt(write_batch, &write_options)
            .map_err(|error| StorageError::DurabilityFailure(error.to_string()))
    }

    fn column_family(&self, name: &str) -> Result<&rocksdb::ColumnFamily, StorageError> {
        self.db
            .cf_handle(name)
            .ok_or_else(|| corrupt(format!("missing {name} column family")))
    }
}

#[cfg(test)]
fn run_synthetic_minor_migration_1_1(
    db: &DB,
    chain_meta: &rocksdb::ColumnFamily,
    interrupt_after_first_step: bool,
) -> Result<(), StorageError> {
    const STEP1_KEY: &[u8] = b"test/migration/step1";
    const STEP2_KEY: &[u8] = b"test/migration/step2";
    const STEP_VALUE: &[u8] = b"applied";

    let from = SCHEMA_VERSION;
    let to = SchemaVersion { major: 1, minor: 1 };
    let expected_marker = encode_migration_marker(from, to);

    match db.get_cf(chain_meta, SCHEMA_MIGRATION_KEY)? {
        Some(bytes) => {
            let marker = decode_migration_marker(&bytes)?;
            if marker != (from, to) {
                return Err(StorageError::CorruptData(
                    "migration marker does not match synthetic 1.0 -> 1.1 migration".to_owned(),
                ));
            }
        }
        None => sync_put(db, chain_meta, SCHEMA_MIGRATION_KEY, &expected_marker)?,
    }

    sync_put(db, chain_meta, STEP1_KEY, STEP_VALUE)?;
    if interrupt_after_first_step {
        return Err(StorageError::DurabilityFailure(
            "injected migration interruption after first step".to_owned(),
        ));
    }

    sync_put(db, chain_meta, STEP2_KEY, STEP_VALUE)?;

    let mut final_batch = WriteBatch::default();
    final_batch.put_cf(chain_meta, SCHEMA_KEY, encode_schema_version(to));
    final_batch.delete_cf(chain_meta, SCHEMA_MIGRATION_KEY);
    sync_write(db, final_batch)
}

#[cfg(test)]
fn sync_put(
    db: &DB,
    column_family: &rocksdb::ColumnFamily,
    key: &[u8],
    value: &[u8],
) -> Result<(), StorageError> {
    let mut batch = WriteBatch::default();
    batch.put_cf(column_family, key, value);
    sync_write(db, batch)
}

#[cfg(test)]
fn sync_write(db: &DB, batch: WriteBatch) -> Result<(), StorageError> {
    let mut write_options = WriteOptions::default();
    write_options.set_sync(true);
    write_options.disable_wal(false);
    db.write_opt(batch, &write_options)
        .map_err(|error| StorageError::DurabilityFailure(error.to_string()))
}

struct EncodedOp {
    column_family: &'static str,
    key: Vec<u8>,
    value: Option<Vec<u8>>,
}

impl EncodedOp {
    fn put(column_family: &'static str, key: impl Into<Vec<u8>>, value: Vec<u8>) -> Self {
        Self {
            column_family,
            key: key.into(),
            value: Some(value),
        }
    }

    fn delete(column_family: &'static str, key: impl Into<Vec<u8>>) -> Self {
        Self {
            column_family,
            key: key.into(),
            value: None,
        }
    }
}

fn encode_operations(operations: Vec<StorageOp>) -> Result<Vec<EncodedOp>, StorageError> {
    let mut encoded = Vec::with_capacity(operations.len());
    for operation in operations {
        match operation {
            StorageOp::PutBlock(block) => {
                let block_id = block.header.block_id();
                encoded.push(EncodedOp::put(
                    CF_BLOCKS,
                    block_id.as_bytes().to_vec(),
                    block.encode(),
                ));
            }
            StorageOp::DeleteBlock(block_id) => {
                encoded.push(EncodedOp::delete(CF_BLOCKS, block_id.as_bytes().to_vec()))
            }
            StorageOp::PutIndex(record) => {
                let block_id = record.header.block_id();
                let value = encode_block_index(&record)?;
                encoded.push(EncodedOp::put(
                    CF_BLOCK_INDEX,
                    block_id.as_bytes().to_vec(),
                    value,
                ));
            }
            StorageOp::PutUtxo(outpoint, entry) => encoded.push(EncodedOp::put(
                CF_UTXO,
                encode_outpoint_key(&outpoint).to_vec(),
                encode_utxo_entry(&entry)?,
            )),
            StorageOp::DeleteUtxo(outpoint) => encoded.push(EncodedOp::delete(
                CF_UTXO,
                encode_outpoint_key(&outpoint).to_vec(),
            )),
            StorageOp::PutUndo(block_id, undo) => encoded.push(EncodedOp::put(
                CF_UNDO,
                block_id.as_bytes().to_vec(),
                encode_block_undo(&undo)?,
            )),
            StorageOp::DeleteUndo(block_id) => {
                encoded.push(EncodedOp::delete(CF_UNDO, block_id.as_bytes().to_vec()))
            }
            StorageOp::SetActiveHeight(height, block_id) => encoded.push(EncodedOp::put(
                CF_CHAIN_META,
                active_height_key(height),
                block_id.as_bytes().to_vec(),
            )),
            StorageOp::DeleteActiveHeight(height) => {
                encoded.push(EncodedOp::delete(CF_CHAIN_META, active_height_key(height)))
            }
            StorageOp::SetTip(block_id, height) => {
                encoded.push(EncodedOp::put(
                    CF_CHAIN_META,
                    ACTIVE_TIP_ID_KEY.to_vec(),
                    block_id.as_bytes().to_vec(),
                ));
                encoded.push(EncodedOp::put(
                    CF_CHAIN_META,
                    ACTIVE_TIP_HEIGHT_KEY.to_vec(),
                    height.to_le_bytes().to_vec(),
                ));
            }
            StorageOp::SetConfigAnchorId(block_id) => encoded.push(EncodedOp::put(
                CF_CHAIN_META,
                CONFIG_ANCHOR_ID_KEY.to_vec(),
                block_id.as_bytes().to_vec(),
            )),
            StorageOp::SetConfigGenesisTimestamp(timestamp) => encoded.push(EncodedOp::put(
                CF_CHAIN_META,
                CONFIG_GENESIS_TIMESTAMP_KEY.to_vec(),
                timestamp.to_le_bytes().to_vec(),
            )),
            StorageOp::SetHealth(health) => encoded.push(EncodedOp::put(
                CF_CHAIN_META,
                HEALTH_STATE_KEY.to_vec(),
                encode_node_health(health).to_vec(),
            )),
            StorageOp::SetPruneCursor(height) => encoded.push(EncodedOp::put(
                CF_CHAIN_META,
                PRUNE_CURSOR_KEY.to_vec(),
                height.to_le_bytes().to_vec(),
            )),
        }
    }
    Ok(encoded)
}

fn decode_hash(bytes: &[u8], context: &str) -> Result<Hash256, StorageError> {
    Hash256::from_slice(bytes).map_err(|error| corrupt(format!("{context}: {error}")))
}

fn decode_u64_le(bytes: &[u8], context: &str) -> Result<u64, StorageError> {
    let exact: [u8; 8] = bytes
        .try_into()
        .map_err(|_| corrupt(format!("{context} must be exactly 8 bytes")))?;
    Ok(u64::from_le_bytes(exact))
}

fn corrupt(message: impl Into<String>) -> StorageError {
    StorageError::CorruptData(message.into())
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
