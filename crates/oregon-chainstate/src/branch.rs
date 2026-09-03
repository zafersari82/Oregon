use oregon_consensus::{ChainWork, PowKeyBlockSource, Target, block_work};
use oregon_primitives::Hash256;
use oregon_storage::{BlockIndexRecord, OregonDb, ValidationStatus};

use crate::ChainStateError;

pub(crate) struct BranchView<'a> {
    db: &'a OregonDb,
    tip: Hash256,
}

impl<'a> BranchView<'a> {
    pub(crate) const fn new(db: &'a OregonDb, tip: Hash256) -> Self {
        Self { db, tip }
    }

    pub(crate) fn ancestor_id_at_height(
        &self,
        height: u64,
    ) -> Result<Option<Hash256>, ChainStateError> {
        let mut current_id = self.tip;
        let mut current = self.checked_index(current_id)?;
        if height > current.height {
            return Ok(None);
        }

        while current.height > height {
            current_id = current.parent;
            current = self.checked_index(current_id)?;
        }

        Ok(Some(current_id))
    }

    pub(crate) fn mtp_window(&self) -> Result<Vec<u64>, ChainStateError> {
        let mut timestamps = Vec::with_capacity(11);
        let mut current_id = self.tip;

        while timestamps.len() < 11 {
            let current = self.checked_index(current_id)?;
            timestamps.push(current.header.timestamp);
            if current.height == 0 {
                break;
            }
            current_id = current.parent;
        }

        Ok(timestamps)
    }

    fn checked_index(&self, block_id: Hash256) -> Result<BlockIndexRecord, ChainStateError> {
        let record = self
            .db
            .get_index(block_id)?
            .ok_or_else(|| corrupt(format!("missing branch index for {block_id:?}")))?;

        if record.validation == ValidationStatus::Invalid {
            return Err(corrupt(format!(
                "invalid block {block_id:?} cannot be used as branch ancestry"
            )));
        }

        if record.height == 0 {
            if record.cumulative_work != ChainWork::zero() {
                return Err(corrupt("height-zero branch anchor has non-zero chainwork"));
            }
            return Ok(record);
        }

        let parent = self
            .db
            .get_index(record.parent)?
            .ok_or_else(|| corrupt(format!("missing parent index for {block_id:?}")))?;
        if parent.validation == ValidationStatus::Invalid {
            return Err(corrupt(format!(
                "block {block_id:?} descends from an invalid parent"
            )));
        }

        let expected_height = parent
            .height
            .checked_add(1)
            .ok_or_else(|| corrupt("branch height overflow"))?;
        if record.height != expected_height {
            return Err(corrupt(format!(
                "branch height {} does not follow parent height {}",
                record.height, parent.height
            )));
        }

        let target = Target::from_le_bytes(record.header.difficulty_commitment)
            .map_err(|error| corrupt(format!("invalid branch target: {error}")))?;
        let mut expected_work = parent.cumulative_work.clone();
        expected_work.add_assign(&block_work(target));
        if record.cumulative_work != expected_work {
            return Err(corrupt("branch cumulative chainwork mismatch"));
        }

        Ok(record)
    }
}

impl PowKeyBlockSource for BranchView<'_> {
    fn validated_block_id_at_height(&self, height: u64) -> Option<Hash256> {
        self.ancestor_id_at_height(height).ok().flatten()
    }
}

fn corrupt(message: impl Into<String>) -> ChainStateError {
    ChainStateError::CorruptState(message.into())
}
