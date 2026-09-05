use oregon_primitives::{BlockHeader, Hash256};

use crate::ChainStateError;
use crate::branch::BranchView;
use crate::state::ChainState;

impl ChainState {
    pub fn chain_id(&self) -> Hash256 {
        self.config.anchor_header.block_id()
    }

    pub fn preferred_header_id_at_height(
        &self,
        height: u64,
    ) -> Result<Option<Hash256>, ChainStateError> {
        if height > self.header_tip.height {
            return Ok(None);
        }
        BranchView::new(&self.db, self.header_tip.block_id).ancestor_id_at_height(height)
    }

    pub fn preferred_header_at_height(
        &self,
        height: u64,
    ) -> Result<Option<BlockHeader>, ChainStateError> {
        let Some(block_id) = self.preferred_header_id_at_height(height)? else {
            return Ok(None);
        };
        let record = self
            .db
            .get_index(block_id)?
            .ok_or_else(|| corrupt("preferred header ancestry index disappeared"))?;
        Ok(Some(record.header))
    }

    pub fn active_id_at_height(&self, height: u64) -> Result<Option<Hash256>, ChainStateError> {
        Ok(self.db.active_id_at_height(height)?)
    }

    pub fn body_retained(&self, block_id: Hash256) -> Result<bool, ChainStateError> {
        Ok(self
            .db
            .get_index(block_id)?
            .is_some_and(|record| record.body_retained))
    }
}

fn corrupt(message: impl Into<String>) -> ChainStateError {
    ChainStateError::CorruptState(message.into())
}
