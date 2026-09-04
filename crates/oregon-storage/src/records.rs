use oregon_consensus::ChainWork;
use oregon_primitives::{BlockHeader, Hash256, write_varint};

use crate::codec::StorageCursor;
use crate::error::StorageError;

const STORAGE_RECORD_VERSION: u8 = 1;
const BLOCK_HEADER_BYTES: usize = 114;
const MAX_CHAINWORK_BYTES: usize = 40;

#[cfg(test)]
pub(crate) const SCHEMA_MIGRATION_KEY: &[u8] = b"schema/migration";
pub(crate) const CONFIG_ANCHOR_ID_KEY: &[u8] = b"config/anchor_id";
pub(crate) const CONFIG_GENESIS_TIMESTAMP_KEY: &[u8] = b"config/genesis_timestamp";
pub(crate) const ACTIVE_TIP_ID_KEY: &[u8] = b"active/tip_id";
pub(crate) const ACTIVE_TIP_HEIGHT_KEY: &[u8] = b"active/tip_height";
pub(crate) const HEALTH_STATE_KEY: &[u8] = b"health/state";
pub(crate) const PRUNE_CURSOR_KEY: &[u8] = b"prune/cursor";
const ACTIVE_HEIGHT_PREFIX: &[u8; 7] = b"active/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationStatus {
    HeaderValidated,
    FullyValidated,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockIndexRecord {
    pub header: BlockHeader,
    pub parent: Hash256,
    pub height: u64,
    pub cumulative_work: ChainWork,
    pub validation: ValidationStatus,
    pub body_retained: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeHealth {
    Healthy,
    ReindexRequired,
}

pub(crate) fn encode_block_index(record: &BlockIndexRecord) -> Result<Vec<u8>, StorageError> {
    if record.parent != record.header.previous_block {
        return Err(corrupt(
            "block index parent does not match header previous block",
        ));
    }

    let chainwork = record.cumulative_work.to_canonical_be_bytes();
    if chainwork.len() > MAX_CHAINWORK_BYTES {
        return Err(corrupt(format!(
            "chainwork encoding length {} exceeds {}",
            chainwork.len(),
            MAX_CHAINWORK_BYTES
        )));
    }

    let header = record.header.encode();
    if header.len() != BLOCK_HEADER_BYTES {
        return Err(corrupt(format!(
            "block header encoding must be {BLOCK_HEADER_BYTES} bytes"
        )));
    }

    let mut bytes = Vec::with_capacity(1 + BLOCK_HEADER_BYTES + 32 + 8 + 1 + chainwork.len() + 2);
    bytes.push(STORAGE_RECORD_VERSION);
    bytes.extend_from_slice(&header);
    bytes.extend_from_slice(record.parent.as_bytes());
    bytes.extend_from_slice(&record.height.to_le_bytes());
    write_varint(chainwork.len() as u64, &mut bytes);
    bytes.extend_from_slice(&chainwork);
    bytes.push(encode_validation_status(record.validation));
    bytes.push(u8::from(record.body_retained));
    Ok(bytes)
}

pub(crate) fn decode_block_index(bytes: &[u8]) -> Result<BlockIndexRecord, StorageError> {
    let mut cursor = StorageCursor::new(bytes);
    let version = cursor.read_u8("block index record version")?;
    if version != STORAGE_RECORD_VERSION {
        return Err(corrupt(format!(
            "unsupported block index record version {version}"
        )));
    }

    let header_bytes = cursor.read_exact(BLOCK_HEADER_BYTES, "block index header")?;
    let header = BlockHeader::decode(header_bytes)
        .map_err(|error| corrupt(format!("invalid block index header: {error:?}")))?;
    let parent = Hash256::from_slice(cursor.read_exact(32, "block index parent")?)
        .map_err(|error| corrupt(format!("invalid block index parent: {error:?}")))?;
    let height = cursor.read_u64("block index height")?;
    let chainwork_len = cursor.read_len(MAX_CHAINWORK_BYTES, "block index chainwork length")?;
    let chainwork_bytes = cursor.read_exact(chainwork_len, "block index chainwork")?;
    let cumulative_work = ChainWork::from_canonical_be_bytes(chainwork_bytes)
        .ok_or_else(|| corrupt("non-canonical block index chainwork"))?;
    let validation = decode_validation_status(cursor.read_u8("block index validation status")?)?;
    let body_retained = match cursor.read_u8("block index body-retained flag")? {
        0 => false,
        1 => true,
        value => return Err(corrupt(format!("invalid body-retained flag {value}"))),
    };
    cursor.finish("block index trailing bytes")?;

    if parent != header.previous_block {
        return Err(corrupt(
            "block index parent does not match header previous block",
        ));
    }

    Ok(BlockIndexRecord {
        header,
        parent,
        height,
        cumulative_work,
        validation,
        body_retained,
    })
}

pub(crate) fn encode_node_health(health: NodeHealth) -> [u8; 2] {
    [
        STORAGE_RECORD_VERSION,
        match health {
            NodeHealth::Healthy => 0,
            NodeHealth::ReindexRequired => 1,
        },
    ]
}

pub(crate) fn decode_node_health(bytes: &[u8]) -> Result<NodeHealth, StorageError> {
    let mut cursor = StorageCursor::new(bytes);
    let version = cursor.read_u8("node health record version")?;
    if version != STORAGE_RECORD_VERSION {
        return Err(corrupt(format!(
            "unsupported node health record version {version}"
        )));
    }
    let health = match cursor.read_u8("node health state")? {
        0 => NodeHealth::Healthy,
        1 => NodeHealth::ReindexRequired,
        value => return Err(corrupt(format!("invalid node health state {value}"))),
    };
    cursor.finish("node health trailing bytes")?;
    Ok(health)
}

pub(crate) fn active_height_key(height: u64) -> [u8; 15] {
    let mut key = [0u8; 15];
    key[..7].copy_from_slice(ACTIVE_HEIGHT_PREFIX);
    key[7..].copy_from_slice(&height.to_be_bytes());
    key
}

fn encode_validation_status(status: ValidationStatus) -> u8 {
    match status {
        ValidationStatus::HeaderValidated => 0,
        ValidationStatus::FullyValidated => 1,
        ValidationStatus::Invalid => 2,
    }
}

fn decode_validation_status(value: u8) -> Result<ValidationStatus, StorageError> {
    match value {
        0 => Ok(ValidationStatus::HeaderValidated),
        1 => Ok(ValidationStatus::FullyValidated),
        2 => Ok(ValidationStatus::Invalid),
        _ => Err(corrupt(format!("invalid block validation status {value}"))),
    }
}

fn corrupt(message: impl Into<String>) -> StorageError {
    StorageError::CorruptData(message.into())
}
