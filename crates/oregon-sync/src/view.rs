use async_trait::async_trait;
use oregon_primitives::{BlockHeader, Hash256};

use crate::SyncViewError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncTip {
    pub block_id: Hash256,
    pub height: u64,
}

#[async_trait]
pub trait ChainSyncView: Send + Sync {
    async fn active_tip(&self) -> Result<SyncTip, SyncViewError>;
    async fn preferred_header_tip(&self) -> Result<SyncTip, SyncViewError>;
    async fn active_id_at_height(&self, height: u64) -> Result<Option<Hash256>, SyncViewError>;
    async fn preferred_header_id_at_height(
        &self,
        height: u64,
    ) -> Result<Option<Hash256>, SyncViewError>;
    async fn preferred_header_at_height(
        &self,
        height: u64,
    ) -> Result<Option<BlockHeader>, SyncViewError>;
    async fn body_retained(&self, block_id: Hash256) -> Result<bool, SyncViewError>;
}
