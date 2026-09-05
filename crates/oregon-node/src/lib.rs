#![forbid(unsafe_code)]

use std::marker::PhantomData;

use oregon_chainstate::{
    AcceptOutcome, ChainState, ChainStateError, HeaderImportOutcome, SessionHealth,
};
use oregon_mempool::{AdmissionOutcome, MempoolConfig, MempoolError};
use oregon_network::Transport;
use oregon_peer::{PeerCommand, PeerId};
use oregon_primitives::{Block, Transaction};
use oregon_protocol::{InventoryItem, InventoryKind};
use oregon_utxo::SpendVerifier;
use thiserror::Error;

mod config;
mod core;
mod network;
mod orchestration;
mod relay;
mod sync_adapter;

pub use config::NodeConfig;
use core::{CoreHandle, spawn_core};
pub use network::{NodeNetwork, NodeNetworkError};
use relay::{RelayState, validated_relay};
pub use relay::ValidatedRelay;
pub use sync_adapter::NodeSyncView;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum NodeQueueError {
    #[error("node core byte budget is exhausted")]
    ByteBudgetExhausted,
    #[error("node core command queue is full")]
    QueueFull,
    #[error("node core worker is closed")]
    Closed,
    #[error("header batch exceeds the node core validation slice")]
    HeaderBatchTooLarge,
}

#[derive(Debug, Error)]
pub enum NodeTransactionError {
    #[error("chainstate is unavailable for mempool mutation: {0:?}")]
    Unavailable(SessionHealth),
    #[error(transparent)]
    Mempool(#[from] MempoolError),
}

#[derive(Debug)]
pub struct BlockSubmission {
    pub result: Result<AcceptOutcome, ChainStateError>,
    pub relay_authorization: Option<ValidatedRelay>,
}

#[derive(Debug)]
pub struct TransactionSubmission {
    pub result: Result<AdmissionOutcome, NodeTransactionError>,
    pub relay_authorization: Option<ValidatedRelay>,
}

pub struct OregonNode<V, T> {
    core: CoreHandle,
    transport: T,
    relay: RelayState,
    _verifier: PhantomData<fn() -> V>,
}

impl<V, T> OregonNode<V, T>
where
    V: SpendVerifier + Send + 'static,
    T: Transport,
{
    pub async fn new(
        state: ChainState,
        mempool_config: MempoolConfig,
        verifier: V,
        transport: T,
    ) -> Result<Self, MempoolError> {
        let core = spawn_core(state, mempool_config, verifier)?;
        Ok(Self {
            core,
            transport,
            relay: RelayState::default(),
            _verifier: PhantomData,
        })
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn sync_view(&self) -> NodeSyncView {
        NodeSyncView::new(self.core.clone())
    }

    pub fn note_peer_inventory(&mut self, peer_id: PeerId, item: InventoryItem) -> bool {
        self.relay.note_peer_inventory(peer_id, item)
    }

    pub fn relay_inventory_commands<I>(
        &mut self,
        source_peer: Option<PeerId>,
        peers: I,
        authorization: ValidatedRelay,
    ) -> Vec<PeerCommand>
    where
        I: IntoIterator<Item = PeerId>,
    {
        self.relay
            .relay_inventory(source_peer, peers, &authorization)
    }

    pub fn request_object_commands(
        &self,
        peer_id: PeerId,
        item: InventoryItem,
    ) -> Vec<PeerCommand> {
        relay::object_request_commands(peer_id, item)
    }

    pub async fn submit_headers(
        &self,
        headers: Vec<oregon_primitives::BlockHeader>,
    ) -> Result<Vec<Result<HeaderImportOutcome, ChainStateError>>, NodeQueueError> {
        self.core.submit_headers(headers).await
    }

    pub async fn submit_block(&self, block: Block) -> Result<BlockSubmission, NodeQueueError> {
        let block_id = block.header.block_id();
        let result = self.core.submit_block(block).await?;
        let relay_authorization = validated_relay(InventoryKind::Block, block_id, &result);
        Ok(BlockSubmission {
            result,
            relay_authorization,
        })
    }

    pub async fn submit_transaction(
        &self,
        transaction: Transaction,
    ) -> Result<TransactionSubmission, NodeQueueError> {
        let txid = transaction.txid();
        let result = self.core.submit_transaction(transaction).await?;
        let relay_authorization = validated_relay(InventoryKind::Transaction, txid, &result);
        Ok(TransactionSubmission {
            result,
            relay_authorization,
        })
    }
}

#[cfg(test)]
mod core_tests;
#[cfg(test)]
mod orchestration_tests;
#[cfg(test)]
mod relay_tests;
#[cfg(test)]
mod sync_adapter_tests;
