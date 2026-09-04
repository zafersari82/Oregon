use oregon_primitives::Block;
use oregon_storage::{StorageBatch, ValidationStatus};
use oregon_utxo::SpendVerifier;

use crate::ChainStateError;
use crate::header;
use crate::state::{AcceptOutcome, ChainState};
use crate::transition::{extend_active, reorganize};

pub(crate) fn accept_block_healthy<V: SpendVerifier>(
    state: &mut ChainState,
    block: Block,
    verifier: &V,
) -> Result<AcceptOutcome, ChainStateError> {
    let block_id = block.header.block_id();
    if let Some(existing) = state.db.get_index(block_id)? {
        if existing.validation == ValidationStatus::Invalid {
            return Err(corrupt("known candidate is marked invalid"));
        }
        return Ok(if block_id == state.tip.block_id {
            AcceptOutcome::Extended
        } else {
            AcceptOutcome::StoredSideChain
        });
    }

    let mut index = header::validate_candidate_header(state, &block.header)?;
    let parent_id = index.parent;
    let height = index.height;
    let cumulative_work = index.cumulative_work.clone();

    if parent_id == state.tip.block_id {
        return extend_active(state, block, height, cumulative_work, verifier);
    }

    index.body_retained = true;
    if cumulative_work > state.tip.cumulative_work {
        return reorganize(state, block, index, verifier);
    }

    let mut batch = StorageBatch::new();
    batch.put_block(block);
    batch.put_index(index);
    state.db.commit_durable(batch)?;

    Ok(AcceptOutcome::StoredSideChain)
}

fn corrupt(message: impl Into<String>) -> ChainStateError {
    ChainStateError::CorruptState(message.into())
}
