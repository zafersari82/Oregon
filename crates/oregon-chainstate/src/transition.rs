use oregon_consensus::ChainWork;
use oregon_primitives::{Block, Hash256};
use oregon_storage::{BlockIndexRecord, NodeHealth, StorageBatch, ValidationStatus};
use oregon_utxo::SpendVerifier;

use crate::ChainStateError;
use crate::header::HeaderTip;
use crate::reorg::{ReorgPlan, discover_fork, load_reorg_plan, reorg_depth_allowed};
use crate::state::{AcceptOutcome, ChainState, SessionHealth, Tip};
use crate::utxo_delta::{
    UtxoDelta, apply_utxo_delta, build_utxo_delta, record_connect_delta, record_disconnect_delta,
};

pub(crate) fn extend_active<V: SpendVerifier>(
    state: &mut ChainState,
    block: Block,
    height: u64,
    cumulative_work: ChainWork,
    verifier: &V,
) -> Result<AcceptOutcome, ChainStateError> {
    let block_id = block.header.block_id();
    let parent_id = block.header.previous_block;
    let mut staged = state.utxos.clone();
    let undo = staged.connect_block(&block, height, &state.config.params, verifier)?;
    let delta = build_utxo_delta(&staged, &undo)?;
    let index = BlockIndexRecord {
        header: block.header.clone(),
        parent: parent_id,
        height,
        cumulative_work: cumulative_work.clone(),
        validation: ValidationStatus::FullyValidated,
        body_retained: true,
    };
    let becomes_preferred = cumulative_work > state.header_tip.cumulative_work;

    let mut batch = StorageBatch::new();
    batch.put_block(block);
    batch.put_index(index);
    batch.put_undo(block_id, undo);
    apply_utxo_delta(&mut batch, delta);
    batch.set_active_height(height, block_id);
    batch.set_tip(block_id, height);
    if becomes_preferred {
        batch.set_preferred_header_tip(block_id, height);
    }
    state.db.commit_durable(batch)?;

    state.utxos = staged;
    state.tip = Tip {
        block_id,
        height,
        cumulative_work: cumulative_work.clone(),
    };
    if becomes_preferred {
        state.header_tip = HeaderTip {
            block_id,
            height,
            cumulative_work,
        };
    }
    Ok(AcceptOutcome::Extended)
}

pub(crate) fn reorganize<V: SpendVerifier>(
    state: &mut ChainState,
    candidate_block: Block,
    candidate_index: BlockIndexRecord,
    verifier: &V,
) -> Result<AcceptOutcome, ChainStateError> {
    let candidate_id = candidate_block.header.block_id();
    let discovery = discover_fork(&state.db, candidate_id, candidate_index)?;
    let depth = state
        .tip
        .height
        .checked_sub(discovery.fork_height)
        .ok_or_else(|| corrupt("candidate fork height exceeds active tip"))?;

    if !reorg_depth_allowed(depth) {
        let mut batch = StorageBatch::new();
        batch.set_health(NodeHealth::ReindexRequired);
        state.db.commit_durable(batch)?;
        state.session_health = SessionHealth::ReindexRequired;
        return Err(ChainStateError::ReindexRequired);
    }

    let plan = load_reorg_plan(
        &state.db,
        state.tip.height,
        discovery,
        candidate_id,
        &candidate_block,
    )?;
    apply_reorg_plan(state, plan, candidate_id, verifier)
}

fn apply_reorg_plan<V: SpendVerifier>(
    state: &mut ChainState,
    plan: ReorgPlan,
    current_candidate_id: Hash256,
    verifier: &V,
) -> Result<AcceptOutcome, ChainStateError> {
    let ReorgPlan {
        fork_height,
        old_active,
        candidate,
    } = plan;
    if candidate.is_empty() {
        return Err(corrupt("reorg candidate path is empty"));
    }

    let mut staged = state.utxos.clone();
    let mut delta = UtxoDelta::new();
    for (_, undo) in old_active {
        record_disconnect_delta(&mut delta, &undo);
        staged.disconnect_block(undo)?;
    }

    let mut new_undos = Vec::with_capacity(candidate.len());
    for (position, node) in candidate.iter().enumerate() {
        match staged.connect_block(
            &node.block,
            node.index.height,
            &state.config.params,
            verifier,
        ) {
            Ok(undo) => {
                record_connect_delta(&mut delta, &staged, &undo)?;
                new_undos.push(undo);
            }
            Err(error) => {
                let mut batch = StorageBatch::new();
                for invalid in &candidate[position..] {
                    let mut index = invalid.index.clone();
                    index.validation = ValidationStatus::Invalid;
                    if invalid.id == current_candidate_id {
                        index.body_retained = false;
                    }
                    batch.put_index(index);
                }
                state.db.commit_durable(batch)?;
                return Err(ChainStateError::Utxo(error));
            }
        }
    }

    let new_tip = candidate
        .last()
        .ok_or_else(|| corrupt("reorg candidate path is empty"))?;
    let new_tip_id = new_tip.id;
    let new_tip_height = new_tip.index.height;
    let new_tip_work = new_tip.index.cumulative_work.clone();

    let mut batch = StorageBatch::new();
    let first_replaced_height = fork_height
        .checked_add(1)
        .ok_or_else(|| corrupt("reorg fork height overflow"))?;
    for height in first_replaced_height..=state.tip.height {
        batch.delete_active_height(height);
    }
    for (node, undo) in candidate.iter().zip(new_undos) {
        let mut index = node.index.clone();
        index.validation = ValidationStatus::FullyValidated;
        index.body_retained = true;
        batch.put_block(node.block.clone());
        batch.put_index(index);
        batch.put_undo(node.id, undo);
        batch.set_active_height(node.index.height, node.id);
    }
    apply_utxo_delta(&mut batch, delta);
    batch.set_tip(new_tip_id, new_tip_height);
    state.db.commit_durable(batch)?;

    state.utxos = staged;
    state.tip = Tip {
        block_id: new_tip_id,
        height: new_tip_height,
        cumulative_work: new_tip_work,
    };
    Ok(AcceptOutcome::Reorganized)
}

fn corrupt(message: impl Into<String>) -> ChainStateError {
    ChainStateError::CorruptState(message.into())
}
