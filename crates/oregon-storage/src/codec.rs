use std::collections::BTreeSet;

use oregon_primitives::{Amount, Decoder, Hash256, OutPoint, TxOutput, write_varint};
use oregon_utxo::{BlockUndo, UtxoEntry};

use crate::error::StorageError;

const STORAGE_RECORD_VERSION: u8 = 1;
const MAX_LOCKING_PROGRAM_BYTES: usize = 65_536;
const MAX_UTXO_RECORD_BYTES: usize = 65_600;
const MAX_UNDO_ENTRIES: usize = 1_000_000;

pub(crate) struct StorageCursor<'a> {
    decoder: Decoder<'a>,
}

impl<'a> StorageCursor<'a> {
    pub(crate) const fn new(input: &'a [u8]) -> Self {
        Self {
            decoder: Decoder::new(input),
        }
    }

    pub(crate) fn read_u8(&mut self, context: &str) -> Result<u8, StorageError> {
        Ok(self.read_exact(1, context)?[0])
    }

    pub(crate) fn read_u64(&mut self, context: &str) -> Result<u64, StorageError> {
        self.decoder
            .read_u64()
            .map_err(|error| primitive_error(context, error))
    }

    pub(crate) fn read_len(&mut self, max: usize, context: &str) -> Result<usize, StorageError> {
        self.decoder
            .read_len(max)
            .map_err(|error| primitive_error(context, error))
    }

    pub(crate) fn read_exact(
        &mut self,
        len: usize,
        context: &str,
    ) -> Result<&'a [u8], StorageError> {
        self.decoder
            .read_bytes(len)
            .map_err(|error| primitive_error(context, error))
    }

    pub(crate) fn finish(self, context: &str) -> Result<(), StorageError> {
        self.decoder
            .finish()
            .map_err(|error| primitive_error(context, error))
    }
}

fn primitive_error(context: &str, error: oregon_primitives::PrimitiveError) -> StorageError {
    StorageError::CorruptData(format!("{context}: {error:?}"))
}

fn corrupt(message: impl Into<String>) -> StorageError {
    StorageError::CorruptData(message.into())
}

pub(crate) fn encode_outpoint_key(outpoint: &OutPoint) -> [u8; 36] {
    let mut key = [0u8; 36];
    key[..32].copy_from_slice(outpoint.txid.as_bytes());
    key[32..].copy_from_slice(&outpoint.index.to_le_bytes());
    key
}

pub(crate) fn decode_outpoint_key(bytes: &[u8]) -> Result<OutPoint, StorageError> {
    if bytes.len() != 36 {
        return Err(corrupt(format!(
            "outpoint key must be exactly 36 bytes, got {}",
            bytes.len()
        )));
    }

    let txid = Hash256::from_slice(&bytes[..32])
        .map_err(|error| primitive_error("invalid outpoint txid", error))?;
    let index = u32::from_le_bytes(
        bytes[32..36]
            .try_into()
            .map_err(|_| corrupt("invalid outpoint index bytes"))?,
    );
    Ok(OutPoint { txid, index })
}

pub(crate) fn encode_utxo_entry(entry: &UtxoEntry) -> Result<Vec<u8>, StorageError> {
    let locking_program = &entry.output.locking_program;
    if locking_program.len() > MAX_LOCKING_PROGRAM_BYTES {
        return Err(corrupt(format!(
            "locking program length {} exceeds {}",
            locking_program.len(),
            MAX_LOCKING_PROGRAM_BYTES
        )));
    }

    let mut bytes = Vec::with_capacity(19 + locking_program.len());
    bytes.push(STORAGE_RECORD_VERSION);
    bytes.extend_from_slice(&entry.output.value.base_units().to_le_bytes());
    bytes.extend_from_slice(&entry.creation_height.to_le_bytes());
    bytes.push(u8::from(entry.is_coinbase));
    write_varint(locking_program.len() as u64, &mut bytes);
    bytes.extend_from_slice(locking_program);
    Ok(bytes)
}

pub(crate) fn decode_utxo_entry(bytes: &[u8]) -> Result<UtxoEntry, StorageError> {
    if bytes.len() > MAX_UTXO_RECORD_BYTES {
        return Err(corrupt(format!(
            "utxo record length {} exceeds {}",
            bytes.len(),
            MAX_UTXO_RECORD_BYTES
        )));
    }

    let mut cursor = StorageCursor::new(bytes);
    let version = cursor.read_u8("utxo record version")?;
    if version != STORAGE_RECORD_VERSION {
        return Err(corrupt(format!(
            "unsupported utxo record version {version}"
        )));
    }

    let value = Amount::from_base_units(cursor.read_u64("utxo value")?)
        .map_err(|error| primitive_error("invalid utxo value", error))?;
    let creation_height = cursor.read_u64("utxo creation height")?;
    let is_coinbase = match cursor.read_u8("utxo coinbase flag")? {
        0 => false,
        1 => true,
        value => return Err(corrupt(format!("invalid utxo coinbase flag {value}"))),
    };
    let program_len = cursor.read_len(MAX_LOCKING_PROGRAM_BYTES, "utxo locking program length")?;
    let locking_program = cursor
        .read_exact(program_len, "utxo locking program")?
        .to_vec();
    cursor.finish("utxo trailing bytes")?;

    Ok(UtxoEntry {
        output: TxOutput {
            value,
            locking_program,
        },
        creation_height,
        is_coinbase,
    })
}

pub(crate) fn encode_block_undo(undo: &BlockUndo) -> Result<Vec<u8>, StorageError> {
    if undo.spent.len() > MAX_UNDO_ENTRIES || undo.created.len() > MAX_UNDO_ENTRIES {
        return Err(corrupt("undo entry count exceeds storage bound"));
    }

    let spent_outpoints: Vec<OutPoint> = undo.spent.iter().map(|(outpoint, _)| *outpoint).collect();
    require_strictly_sorted_outpoints(&spent_outpoints, "spent undo outpoints")?;
    require_strictly_sorted_outpoints(&undo.created, "created undo outpoints")?;

    let spent_set: BTreeSet<OutPoint> = spent_outpoints.iter().copied().collect();
    if undo.created.iter().any(|outpoint| spent_set.contains(outpoint)) {
        return Err(corrupt(
            "undo outpoint appears in both spent and created sets",
        ));
    }

    let mut bytes = Vec::new();
    bytes.push(STORAGE_RECORD_VERSION);
    write_varint(undo.spent.len() as u64, &mut bytes);
    for (outpoint, entry) in &undo.spent {
        bytes.extend_from_slice(&encode_outpoint_key(outpoint));
        let encoded_entry = encode_utxo_entry(entry)?;
        write_varint(encoded_entry.len() as u64, &mut bytes);
        bytes.extend_from_slice(&encoded_entry);
    }

    write_varint(undo.created.len() as u64, &mut bytes);
    for outpoint in &undo.created {
        bytes.extend_from_slice(&encode_outpoint_key(outpoint));
    }
    Ok(bytes)
}

pub(crate) fn decode_block_undo(bytes: &[u8]) -> Result<BlockUndo, StorageError> {
    let mut cursor = StorageCursor::new(bytes);
    let version = cursor.read_u8("undo record version")?;
    if version != STORAGE_RECORD_VERSION {
        return Err(corrupt(format!(
            "unsupported undo record version {version}"
        )));
    }

    let spent_count = cursor.read_len(MAX_UNDO_ENTRIES, "undo spent count")?;
    let mut spent = Vec::with_capacity(spent_count);
    let mut spent_outpoints = BTreeSet::new();
    let mut previous_spent = None;
    for _ in 0..spent_count {
        let key = read_outpoint_key(&mut cursor, "undo spent outpoint")?;
        let outpoint = decode_outpoint_key(&key)?;
        require_next_outpoint(previous_spent, outpoint, "spent undo outpoints")?;
        previous_spent = Some(outpoint);
        spent_outpoints.insert(outpoint);

        let entry_len = cursor.read_len(MAX_UTXO_RECORD_BYTES, "undo utxo record length")?;
        let entry_bytes = cursor.read_exact(entry_len, "undo utxo record")?;
        let entry = decode_utxo_entry(entry_bytes)?;
        spent.push((outpoint, entry));
    }

    let created_count = cursor.read_len(MAX_UNDO_ENTRIES, "undo created count")?;
    let mut created = Vec::with_capacity(created_count);
    let mut previous_created = None;
    for _ in 0..created_count {
        let key = read_outpoint_key(&mut cursor, "undo created outpoint")?;
        let outpoint = decode_outpoint_key(&key)?;
        require_next_outpoint(previous_created, outpoint, "created undo outpoints")?;
        previous_created = Some(outpoint);
        if spent_outpoints.contains(&outpoint) {
            return Err(corrupt(
                "undo outpoint appears in both spent and created sets",
            ));
        }
        created.push(outpoint);
    }

    cursor.finish("undo trailing bytes")?;
    Ok(BlockUndo { spent, created })
}

fn read_outpoint_key(
    cursor: &mut StorageCursor<'_>,
    context: &str,
) -> Result<[u8; 36], StorageError> {
    let bytes = cursor.read_exact(36, context)?;
    bytes
        .try_into()
        .map_err(|_| corrupt(format!("{context} must be 36 bytes")))
}

fn require_strictly_sorted_outpoints(
    outpoints: &[OutPoint],
    context: &str,
) -> Result<(), StorageError> {
    if outpoints.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(corrupt(format!("{context} are not strictly sorted")));
    }
    Ok(())
}

fn require_next_outpoint(
    previous: Option<OutPoint>,
    current: OutPoint,
    context: &str,
) -> Result<(), StorageError> {
    if previous.is_some_and(|value| value >= current) {
        return Err(corrupt(format!("{context} are not strictly sorted")));
    }
    Ok(())
}
