use oregon_primitives::{Block, Hash256, OutPoint};
use oregon_utxo::{BlockUndo, UtxoEntry};

use crate::{BlockIndexRecord, NodeHealth};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurabilityMode {
    Sync,
    NoSync,
}

#[derive(Debug, Default)]
pub struct StorageBatch {
    pub(crate) operations: Vec<StorageOp>,
}

impl StorageBatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put_block(&mut self, block: Block) {
        self.operations.push(StorageOp::PutBlock(block));
    }

    pub fn delete_block(&mut self, block_id: Hash256) {
        self.operations.push(StorageOp::DeleteBlock(block_id));
    }

    pub fn put_index(&mut self, record: BlockIndexRecord) {
        self.operations.push(StorageOp::PutIndex(record));
    }

    pub fn put_utxo(&mut self, outpoint: OutPoint, entry: UtxoEntry) {
        self.operations.push(StorageOp::PutUtxo(outpoint, entry));
    }

    pub fn delete_utxo(&mut self, outpoint: OutPoint) {
        self.operations.push(StorageOp::DeleteUtxo(outpoint));
    }

    pub fn put_undo(&mut self, block_id: Hash256, undo: BlockUndo) {
        self.operations.push(StorageOp::PutUndo(block_id, undo));
    }

    pub fn delete_undo(&mut self, block_id: Hash256) {
        self.operations.push(StorageOp::DeleteUndo(block_id));
    }

    pub fn set_active_height(&mut self, height: u64, block_id: Hash256) {
        self.operations
            .push(StorageOp::SetActiveHeight(height, block_id));
    }

    pub fn delete_active_height(&mut self, height: u64) {
        self.operations.push(StorageOp::DeleteActiveHeight(height));
    }

    pub fn set_tip(&mut self, block_id: Hash256, height: u64) {
        self.operations.push(StorageOp::SetTip(block_id, height));
    }

    pub fn set_config_anchor_id(&mut self, block_id: Hash256) {
        self.operations.push(StorageOp::SetConfigAnchorId(block_id));
    }

    pub fn set_config_genesis_timestamp(&mut self, timestamp: u64) {
        self.operations
            .push(StorageOp::SetConfigGenesisTimestamp(timestamp));
    }

    pub fn set_health(&mut self, health: NodeHealth) {
        self.operations.push(StorageOp::SetHealth(health));
    }

    pub fn set_prune_cursor(&mut self, height: u64) {
        self.operations.push(StorageOp::SetPruneCursor(height));
    }
}

#[derive(Debug)]
pub(crate) enum StorageOp {
    PutBlock(Block),
    DeleteBlock(Hash256),
    PutIndex(BlockIndexRecord),
    PutUtxo(OutPoint, UtxoEntry),
    DeleteUtxo(OutPoint),
    PutUndo(Hash256, BlockUndo),
    DeleteUndo(Hash256),
    SetActiveHeight(u64, Hash256),
    DeleteActiveHeight(u64),
    SetTip(Hash256, u64),
    SetConfigAnchorId(Hash256),
    SetConfigGenesisTimestamp(u64),
    SetHealth(NodeHealth),
    SetPruneCursor(u64),
}
