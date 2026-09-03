use oregon_storage::{OregonDb, StorageBatch, ValidationStatus};

use crate::ChainStateError;
use crate::reorg::discover_fork;
use crate::state::{ChainState, REORG_WINDOW, SessionHealth};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PruneReport {
    pub deleted_bodies: u64,
    pub deleted_undos: u64,
}

pub(crate) fn retained_active_floor(height: u64) -> u64 {
    height.saturating_sub(REORG_WINDOW - 1)
}

pub(crate) fn prune_cursor_for_tip(height: u64) -> u64 {
    height.saturating_sub(REORG_WINDOW)
}

pub(crate) fn should_retain_body(
    block_height: u64,
    common_fork_height: u64,
    active_tip_height: u64,
) -> bool {
    if block_height < retained_active_floor(active_tip_height) {
        return false;
    }
    active_tip_height
        .checked_sub(common_fork_height)
        .is_some_and(|depth| depth <= REORG_WINDOW)
}

pub(crate) fn should_retain_undo(
    is_active: bool,
    block_height: u64,
    active_tip_height: u64,
) -> bool {
    is_active
        && block_height <= active_tip_height
        && block_height >= retained_active_floor(active_tip_height)
}

pub(crate) fn plan_prune(
    db: &OregonDb,
    active_tip_height: u64,
) -> Result<(StorageBatch, PruneReport), ChainStateError> {
    let active_floor = retained_active_floor(active_tip_height);
    let mut batch = StorageBatch::new();
    let mut report = PruneReport::default();

    for (block_id, mut index) in db.iter_body_retained_indices()? {
        let is_active = db.active_id_at_height(index.height)? == Some(block_id);
        if is_active && index.validation != ValidationStatus::FullyValidated {
            return Err(corrupt(format!(
                "active block at height {} is not fully validated during pruning",
                index.height
            )));
        }

        if db.get_block(block_id)?.is_none() {
            return Err(corrupt(format!(
                "block index marks body retained but body is missing for {block_id:?}"
            )));
        }

        let retain_body =
            if index.validation == ValidationStatus::Invalid || index.height < active_floor {
                false
            } else {
                let common_fork_height = if is_active {
                    index.height
                } else {
                    discover_fork(db, block_id, index.clone())?.fork_height
                };
                should_retain_body(index.height, common_fork_height, active_tip_height)
            };

        if !retain_body {
            batch.delete_block(block_id);
            index.body_retained = false;
            batch.put_index(index.clone());
            report.deleted_bodies = report
                .deleted_bodies
                .checked_add(1)
                .ok_or_else(|| corrupt("pruned body count overflow"))?;
        }

        let retain_undo = should_retain_undo(is_active, index.height, active_tip_height);
        match db.get_undo(block_id)? {
            Some(_) if !retain_undo => {
                batch.delete_undo(block_id);
                report.deleted_undos = report
                    .deleted_undos
                    .checked_add(1)
                    .ok_or_else(|| corrupt("pruned undo count overflow"))?;
            }
            None if retain_undo => return Err(ChainStateError::MissingUndo(block_id)),
            _ => {}
        }
    }

    batch.set_prune_cursor(prune_cursor_for_tip(active_tip_height));
    Ok((batch, report))
}

impl ChainState {
    pub fn prune(&mut self) -> Result<PruneReport, ChainStateError> {
        match self.session_health() {
            SessionHealth::Healthy => {}
            SessionHealth::StorageFaulted => return Err(ChainStateError::StorageFaulted),
            SessionHealth::ReindexRequired => return Err(ChainStateError::ReindexRequired),
        }

        let tip_height = self.tip().height;
        let (batch, report) = plan_prune(self.storage(), tip_height)?;
        self.storage().commit_maintenance(batch)?;
        Ok(report)
    }
}

fn corrupt(message: impl Into<String>) -> ChainStateError {
    ChainStateError::CorruptState(message.into())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use oregon_consensus::ChainWork;
    use oregon_primitives::{
        Amount, Block, BlockHeader, Hash256, Transaction, TxInput, TxOutput, transaction_root,
    };
    use oregon_storage::{BlockIndexRecord, OregonDb, StorageBatch, ValidationStatus};
    use oregon_utxo::BlockUndo;

    use super::{
        plan_prune, prune_cursor_for_tip, retained_active_floor, should_retain_body,
        should_retain_undo,
    };
    use crate::state::REORG_WINDOW;

    fn test_path() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("oregon-task9-prune-{}-{n}", std::process::id()))
    }

    fn stored_block(parent: Hash256, height: u64) -> Block {
        let transaction = Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_txid: Hash256::from_bytes([0; 32]),
                previous_output_index: u32::MAX,
                sequence: u32::MAX,
                witness: vec![vec![height as u8]],
            }],
            outputs: vec![TxOutput {
                value: Amount::from_base_units(1).unwrap(),
                locking_program: vec![0x51],
            }],
            lock_time: 0,
        };
        let transactions = vec![transaction];
        Block {
            header: BlockHeader {
                version: 1,
                previous_block: parent,
                transaction_root: transaction_root(&transactions).unwrap(),
                timestamp: 1_800_000_000 + height * 300,
                difficulty_commitment: [0xff; 32],
                nonce: 90_000 + height,
            },
            transactions,
        }
    }

    #[test]
    fn active_floor_retains_8064_blocks_and_saturates_at_zero() {
        assert_eq!(retained_active_floor(0), 0);
        assert_eq!(retained_active_floor(REORG_WINDOW - 1), 0);
        assert_eq!(retained_active_floor(REORG_WINDOW), 1);

        let tip = 20_000;
        let floor = retained_active_floor(tip);
        assert_eq!(floor, tip - (REORG_WINDOW - 1));
        assert_eq!(floor, 11_937);
        assert_eq!(tip - floor + 1, REORG_WINDOW);
    }

    #[test]
    fn prune_cursor_is_highest_active_height_eligible_for_pruning() {
        assert_eq!(prune_cursor_for_tip(0), 0);
        assert_eq!(prune_cursor_for_tip(REORG_WINDOW - 1), 0);
        assert_eq!(prune_cursor_for_tip(REORG_WINDOW), 0);
        assert_eq!(prune_cursor_for_tip(REORG_WINDOW + 1), 1);
        assert_eq!(prune_cursor_for_tip(20_000), 11_936);
    }

    #[test]
    fn body_retention_requires_live_height_and_permitted_common_fork_depth() {
        let tip = 20_000;
        let floor = retained_active_floor(tip);
        let deepest_permitted_fork = tip - REORG_WINDOW;

        assert!(should_retain_body(floor, deepest_permitted_fork, tip));
        assert!(!should_retain_body(floor - 1, deepest_permitted_fork, tip));
        assert!(!should_retain_body(floor, deepest_permitted_fork - 1, tip));
        assert!(should_retain_body(tip, tip, tip));
    }

    #[test]
    fn undo_is_retained_only_for_active_blocks_inside_live_window() {
        let tip = 20_000;
        let floor = retained_active_floor(tip);

        assert!(should_retain_undo(true, floor, tip));
        assert!(should_retain_undo(true, tip, tip));
        assert!(!should_retain_undo(true, floor - 1, tip));
        assert!(!should_retain_undo(false, floor, tip));
    }

    #[test]
    fn pruning_deletes_only_eligible_active_body_and_undo_and_is_idempotent() {
        let path = test_path();
        std::fs::create_dir_all(&path).unwrap();
        let db = OregonDb::open(&path).unwrap();

        let block1 = stored_block(Hash256::from_bytes([0; 32]), 1);
        let id1 = block1.header.block_id();
        let block2 = stored_block(id1, 2);
        let id2 = block2.header.block_id();
        let undo = BlockUndo {
            spent: Vec::new(),
            created: Vec::new(),
        };

        let mut seed = StorageBatch::new();
        seed.put_block(block1.clone());
        seed.put_index(BlockIndexRecord {
            header: block1.header.clone(),
            parent: block1.header.previous_block,
            height: 1,
            cumulative_work: ChainWork::zero(),
            validation: ValidationStatus::FullyValidated,
            body_retained: true,
        });
        seed.put_undo(id1, undo.clone());
        seed.set_active_height(1, id1);
        seed.put_block(block2.clone());
        seed.put_index(BlockIndexRecord {
            header: block2.header.clone(),
            parent: block2.header.previous_block,
            height: 2,
            cumulative_work: ChainWork::zero(),
            validation: ValidationStatus::FullyValidated,
            body_retained: true,
        });
        seed.put_undo(id2, undo);
        seed.set_active_height(2, id2);
        seed.set_prune_cursor(0);
        db.commit_durable(seed).unwrap();

        let tip = REORG_WINDOW + 1;
        let (batch, report) = plan_prune(&db, tip).unwrap();
        assert_eq!(report.deleted_bodies, 1);
        assert_eq!(report.deleted_undos, 1);
        db.commit_maintenance(batch).unwrap();

        assert!(db.get_block(id1).unwrap().is_none());
        assert!(db.get_undo(id1).unwrap().is_none());
        assert!(!db.get_index(id1).unwrap().unwrap().body_retained);
        assert!(db.get_block(id2).unwrap().is_some());
        assert!(db.get_undo(id2).unwrap().is_some());
        assert!(db.get_index(id2).unwrap().unwrap().body_retained);
        assert_eq!(db.prune_cursor().unwrap(), Some(1));

        let (second_batch, second_report) = plan_prune(&db, tip).unwrap();
        assert_eq!(second_report.deleted_bodies, 0);
        assert_eq!(second_report.deleted_undos, 0);
        db.commit_maintenance(second_batch).unwrap();
        assert_eq!(db.prune_cursor().unwrap(), Some(1));

        drop(db);
        let _ = std::fs::remove_dir_all(path);
    }
}
