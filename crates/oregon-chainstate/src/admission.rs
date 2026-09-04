use oregon_consensus::{
    ConsensusError, HeaderContext, validate_header_pow, validate_header_pre_pow,
};
use oregon_pow::{LightEngine, derive_randomx_key, key_block_height};
use oregon_primitives::Block;
use oregon_storage::{BlockIndexRecord, StorageBatch, ValidationStatus};
use oregon_utxo::SpendVerifier;

use crate::ChainStateError;
use crate::branch::BranchView;
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

    let parent_id = block.header.previous_block;
    let parent = state
        .db
        .get_index(parent_id)?
        .ok_or(ChainStateError::UnknownParent(parent_id))?;
    if parent.validation == ValidationStatus::Invalid {
        return Err(corrupt("candidate parent is marked invalid"));
    }

    let branch = BranchView::new(&state.db, parent_id);
    let mtp_window = branch.mtp_window()?;
    let height = parent
        .height
        .checked_add(1)
        .ok_or_else(|| corrupt("candidate height overflow"))?;
    let facts = validate_header_pre_pow(
        &block.header,
        &HeaderContext {
            height,
            parent: &parent.header,
            genesis_timestamp: state.config.genesis_timestamp,
            mtp_window: &mtp_window,
        },
        &state.config.params,
    )?;

    let key_height = key_block_height(height);
    let key_block_id = branch
        .ancestor_id_at_height(key_height)?
        .ok_or(ConsensusError::PowKeyBlockUnavailable)?;
    let mut engine = LightEngine::new(derive_randomx_key(key_block_id))?;
    validate_header_pow(&block.header, &facts, &branch, &mut engine)?;

    let mut cumulative_work = parent.cumulative_work.clone();
    cumulative_work.add_assign(&facts.work());

    if parent_id == state.tip.block_id {
        return extend_active(state, block, height, cumulative_work, verifier);
    }

    let index = BlockIndexRecord {
        header: block.header.clone(),
        parent: parent_id,
        height,
        cumulative_work: cumulative_work.clone(),
        validation: ValidationStatus::HeaderValidated,
        body_retained: true,
    };

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
