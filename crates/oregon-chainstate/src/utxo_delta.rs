use std::collections::BTreeMap;

use oregon_primitives::OutPoint;
use oregon_storage::StorageBatch;
use oregon_utxo::{BlockUndo, UtxoEntry, UtxoState};

use crate::ChainStateError;

pub(crate) type UtxoDelta = BTreeMap<OutPoint, Option<UtxoEntry>>;

pub(crate) fn build_utxo_delta(
    staged: &UtxoState,
    undo: &BlockUndo,
) -> Result<UtxoDelta, ChainStateError> {
    let mut delta = BTreeMap::new();
    for (outpoint, _) in &undo.spent {
        delta.insert(*outpoint, None);
    }
    for outpoint in &undo.created {
        let entry = staged
            .get(outpoint)
            .cloned()
            .ok_or_else(|| corrupt("created outpoint missing from staged UTXO state"))?;
        delta.insert(*outpoint, Some(entry));
    }
    Ok(delta)
}

pub(crate) fn record_disconnect_delta(delta: &mut UtxoDelta, undo: &BlockUndo) {
    for outpoint in &undo.created {
        delta.insert(*outpoint, None);
    }
    for (outpoint, entry) in &undo.spent {
        delta.insert(*outpoint, Some(entry.clone()));
    }
}

pub(crate) fn record_connect_delta(
    delta: &mut UtxoDelta,
    staged: &UtxoState,
    undo: &BlockUndo,
) -> Result<(), ChainStateError> {
    for (outpoint, _) in &undo.spent {
        delta.insert(*outpoint, None);
    }
    for outpoint in &undo.created {
        let entry = staged
            .get(outpoint)
            .cloned()
            .ok_or_else(|| corrupt("created outpoint missing from staged reorg UTXO state"))?;
        delta.insert(*outpoint, Some(entry));
    }
    Ok(())
}

pub(crate) fn apply_utxo_delta(batch: &mut StorageBatch, delta: UtxoDelta) {
    for (outpoint, entry) in delta {
        match entry {
            Some(entry) => batch.put_utxo(outpoint, entry),
            None => batch.delete_utxo(outpoint),
        }
    }
}

fn corrupt(message: impl Into<String>) -> ChainStateError {
    ChainStateError::CorruptState(message.into())
}
