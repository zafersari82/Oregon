use oregon_consensus::{Target, block_work};
use oregon_primitives::{Block, Hash256};
use oregon_storage::{BlockIndexRecord, OregonDb, ValidationStatus};
use oregon_utxo::BlockUndo;

use crate::ChainStateError;
use crate::state::REORG_WINDOW;

#[derive(Debug, Clone)]
pub(crate) struct CandidateNode {
    pub(crate) id: Hash256,
    pub(crate) index: BlockIndexRecord,
}

#[derive(Debug)]
pub(crate) struct ForkDiscovery {
    pub(crate) fork_height: u64,
    pub(crate) candidate: Vec<CandidateNode>,
}

#[derive(Debug)]
pub(crate) struct ReorgCandidate {
    pub(crate) id: Hash256,
    pub(crate) index: BlockIndexRecord,
    pub(crate) block: Block,
}

#[derive(Debug)]
pub(crate) struct ReorgPlan {
    pub(crate) fork_height: u64,
    pub(crate) old_active: Vec<(Hash256, BlockUndo)>,
    pub(crate) candidate: Vec<ReorgCandidate>,
}

pub(crate) fn reorg_depth_allowed(depth: u64) -> bool {
    depth <= REORG_WINDOW
}

pub(crate) fn discover_fork(
    db: &OregonDb,
    candidate_id: Hash256,
    candidate_index: BlockIndexRecord,
) -> Result<ForkDiscovery, ChainStateError> {
    validate_index_identity(candidate_id, &candidate_index)?;

    let mut current_id = candidate_id;
    let mut current = candidate_index;
    let mut candidate = Vec::new();

    loop {
        if db.active_id_at_height(current.height)? == Some(current_id) {
            candidate.reverse();
            return Ok(ForkDiscovery {
                fork_height: current.height,
                candidate,
            });
        }

        if current.validation == ValidationStatus::Invalid {
            return Err(corrupt(format!(
                "invalid candidate ancestry at {current_id:?}"
            )));
        }
        if current.height == 0 {
            return Err(corrupt("candidate branch does not share the active anchor"));
        }

        candidate.push(CandidateNode {
            id: current_id,
            index: current.clone(),
        });

        let parent_id = current.parent;
        let parent = db
            .get_index(parent_id)?
            .ok_or_else(|| corrupt(format!("missing candidate parent index {parent_id:?}")))?;
        validate_parent_child(parent_id, &parent, current_id, &current)?;
        current_id = parent_id;
        current = parent;
    }
}

pub(crate) fn load_reorg_plan(
    db: &OregonDb,
    active_tip_height: u64,
    discovery: ForkDiscovery,
    current_candidate_id: Hash256,
    current_candidate_block: &Block,
) -> Result<ReorgPlan, ChainStateError> {
    let mut old_active = Vec::new();
    for height in (discovery.fork_height + 1..=active_tip_height).rev() {
        let block_id = db
            .active_id_at_height(height)?
            .ok_or_else(|| corrupt(format!("missing active mapping at height {height}")))?;
        let undo = db
            .get_undo(block_id)?
            .ok_or(ChainStateError::MissingUndo(block_id))?;
        old_active.push((block_id, undo));
    }

    let mut candidate = Vec::with_capacity(discovery.candidate.len());
    for node in discovery.candidate {
        let block = if node.id == current_candidate_id {
            current_candidate_block.clone()
        } else {
            if !node.index.body_retained {
                return Err(ChainStateError::MissingBlockBody(node.id));
            }
            db.get_block(node.id)?
                .ok_or(ChainStateError::MissingBlockBody(node.id))?
        };
        if block.header.block_id() != node.id || block.header != node.index.header {
            return Err(corrupt(format!(
                "candidate body/header identity mismatch for {:?}",
                node.id
            )));
        }
        candidate.push(ReorgCandidate {
            id: node.id,
            index: node.index,
            block,
        });
    }

    Ok(ReorgPlan {
        fork_height: discovery.fork_height,
        old_active,
        candidate,
    })
}

fn validate_index_identity(
    block_id: Hash256,
    record: &BlockIndexRecord,
) -> Result<(), ChainStateError> {
    if record.header.block_id() != block_id || record.parent != record.header.previous_block {
        return Err(corrupt(format!(
            "block index identity mismatch for {block_id:?}"
        )));
    }
    Ok(())
}

fn validate_parent_child(
    parent_id: Hash256,
    parent: &BlockIndexRecord,
    child_id: Hash256,
    child: &BlockIndexRecord,
) -> Result<(), ChainStateError> {
    validate_index_identity(parent_id, parent)?;
    validate_index_identity(child_id, child)?;
    if parent.validation == ValidationStatus::Invalid {
        return Err(corrupt(format!(
            "candidate block {child_id:?} descends from invalid parent {parent_id:?}"
        )));
    }
    if child.parent != parent_id {
        return Err(corrupt(format!(
            "candidate parent mismatch for {child_id:?}"
        )));
    }
    let expected_height = parent
        .height
        .checked_add(1)
        .ok_or_else(|| corrupt("candidate branch height overflow"))?;
    if child.height != expected_height {
        return Err(corrupt(format!(
            "candidate height {} does not follow parent height {}",
            child.height, parent.height
        )));
    }

    let target = Target::from_le_bytes(child.header.difficulty_commitment)
        .map_err(|error| corrupt(format!("invalid candidate target: {error}")))?;
    let mut expected_work = parent.cumulative_work.clone();
    expected_work.add_assign(&block_work(target));
    if child.cumulative_work != expected_work {
        return Err(corrupt("candidate cumulative chainwork mismatch"));
    }
    Ok(())
}

fn corrupt(message: impl Into<String>) -> ChainStateError {
    ChainStateError::CorruptState(message.into())
}

#[cfg(test)]
mod tests {
    use super::reorg_depth_allowed;

    #[test]
    fn reorg_window_accepts_8064_and_rejects_8065() {
        assert!(reorg_depth_allowed(8_064));
        assert!(!reorg_depth_allowed(8_065));
    }
}
