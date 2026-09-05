use async_trait::async_trait;
use oregon_primitives::{BlockHeader, Hash256};
use oregon_sync::{ChainSyncView, SyncTip, SyncViewError};

use crate::core::CoreHandle;

#[derive(Clone)]
pub struct NodeSyncView {
    core: CoreHandle,
}

impl NodeSyncView {
    pub(crate) fn new(core: CoreHandle) -> Self {
        Self { core }
    }
}

#[async_trait]
impl ChainSyncView for NodeSyncView {
    async fn active_tip(&self) -> Result<SyncTip, SyncViewError> {
        let (block_id, height) = self
            .core
            .read_active_tip()
            .await
            .map_err(|_| SyncViewError::Unavailable)?;
        Ok(SyncTip { block_id, height })
    }

    async fn preferred_header_tip(&self) -> Result<SyncTip, SyncViewError> {
        let (block_id, height) = self
            .core
            .read_preferred_header_tip()
            .await
            .map_err(|_| SyncViewError::Unavailable)?;
        Ok(SyncTip { block_id, height })
    }

    async fn active_id_at_height(&self, height: u64) -> Result<Option<Hash256>, SyncViewError> {
        self.core
            .read_active_id_at_height(height)
            .await
            .map_err(|_| SyncViewError::Unavailable)?
            .map_err(|_| SyncViewError::Unavailable)
    }

    async fn preferred_header_id_at_height(
        &self,
        height: u64,
    ) -> Result<Option<Hash256>, SyncViewError> {
        self.core
            .read_preferred_header_id_at_height(height)
            .await
            .map_err(|_| SyncViewError::Unavailable)?
            .map_err(|_| SyncViewError::Unavailable)
    }

    async fn preferred_header_at_height(
        &self,
        height: u64,
    ) -> Result<Option<BlockHeader>, SyncViewError> {
        self.core
            .read_preferred_header_at_height(height)
            .await
            .map_err(|_| SyncViewError::Unavailable)?
            .map_err(|_| SyncViewError::Unavailable)
    }

    async fn body_retained(&self, block_id: Hash256) -> Result<bool, SyncViewError> {
        self.core
            .read_body_retained(block_id)
            .await
            .map_err(|_| SyncViewError::Unavailable)?
            .map_err(|_| SyncViewError::Unavailable)
    }
}
