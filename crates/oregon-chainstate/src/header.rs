use oregon_consensus::{
    ChainWork, ConsensusError, HeaderContext, validate_header_pow, validate_header_pre_pow,
};
use oregon_pow::{LightEngine, derive_randomx_key, key_block_height};
use oregon_primitives::{BlockHeader, Hash256};
use oregon_storage::{BlockIndexRecord, ValidationStatus};

use crate::ChainStateError;
use crate::branch::BranchView;
use crate::state::ChainState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderTip {
    pub block_id: Hash256,
    pub height: u64,
    pub cumulative_work: ChainWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderImportStatus {
    Known,
    Stored,
    Preferred,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderImportOutcome {
    pub block_id: Hash256,
    pub height: u64,
    pub status: HeaderImportStatus,
    pub preferred_tip: HeaderTip,
}

pub(crate) fn validate_candidate_header(
    state: &ChainState,
    header: &BlockHeader,
) -> Result<BlockIndexRecord, ChainStateError> {
    let parent_id = header.previous_block;
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
        header,
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
    validate_header_pow(header, &facts, &branch, &mut engine)?;

    let mut cumulative_work = parent.cumulative_work.clone();
    cumulative_work.add_assign(&facts.work());

    Ok(BlockIndexRecord {
        header: header.clone(),
        parent: parent_id,
        height,
        cumulative_work,
        validation: ValidationStatus::HeaderValidated,
        body_retained: false,
    })
}

pub(crate) fn accept_header_healthy(
    _state: &mut ChainState,
    _header: BlockHeader,
) -> Result<HeaderImportOutcome, ChainStateError> {
    Err(corrupt("header import behavior is not implemented yet"))
}

fn corrupt(message: impl Into<String>) -> ChainStateError {
    ChainStateError::CorruptState(message.into())
}
