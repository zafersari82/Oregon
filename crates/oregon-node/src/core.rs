use std::sync::Arc;

use oregon_chainstate::{
    AcceptOutcome, ChainState, ChainStateError, HeaderImportOutcome, SessionHealth,
};
use oregon_mempool::{AdmissionOutcome, ChainBase, Mempool, MempoolConfig, MempoolError};
use oregon_primitives::{Block, BlockHeader, Hash256, Transaction};
use oregon_utxo::SpendVerifier;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot};

#[cfg(test)]
use oregon_sync::SyncTip;

use crate::orchestration::reconcile_after_acceptance;
use crate::{NodeQueueError, NodeTransactionError};

pub(crate) const MAX_CORE_COMMANDS: usize = 64;
pub(crate) const MAX_CORE_COMMAND_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const HEADER_VALIDATION_SLICE: usize = 16;
const CORE_READ_COMMAND_BYTES: usize = 1;

#[derive(Clone)]
pub(crate) struct CoreHandle {
    tx: mpsc::Sender<CoreEnvelope>,
    bytes: Arc<Semaphore>,
}

pub(crate) struct CoreEnvelope {
    command: CoreCommand,
    _bytes: OwnedSemaphorePermit,
}

enum CoreCommand {
    Headers {
        headers: Vec<BlockHeader>,
        response: oneshot::Sender<Vec<Result<HeaderImportOutcome, ChainStateError>>>,
    },
    Block {
        block: Block,
        response: oneshot::Sender<Result<AcceptOutcome, ChainStateError>>,
    },
    Transaction {
        transaction: Transaction,
        response: oneshot::Sender<Result<AdmissionOutcome, NodeTransactionError>>,
    },
    ActiveTip {
        response: oneshot::Sender<(Hash256, u64)>,
    },
    PreferredHeaderTip {
        response: oneshot::Sender<(Hash256, u64)>,
    },
    ActiveIdAtHeight {
        height: u64,
        response: oneshot::Sender<Result<Option<Hash256>, ChainStateError>>,
    },
    PreferredHeaderIdAtHeight {
        height: u64,
        response: oneshot::Sender<Result<Option<Hash256>, ChainStateError>>,
    },
    PreferredHeaderAtHeight {
        height: u64,
        response: oneshot::Sender<Result<Option<BlockHeader>, ChainStateError>>,
    },
    BodyRetained {
        block_id: Hash256,
        response: oneshot::Sender<Result<bool, ChainStateError>>,
    },
    #[cfg(test)]
    TestBytes,
    #[cfg(test)]
    ProbeThread(oneshot::Sender<std::thread::ThreadId>),
}

impl CoreHandle {
    fn try_acquire_bytes(&self, bytes: usize) -> Result<OwnedSemaphorePermit, NodeQueueError> {
        if bytes > MAX_CORE_COMMAND_BYTES {
            return Err(NodeQueueError::ByteBudgetExhausted);
        }
        Arc::clone(&self.bytes)
            .try_acquire_many_owned(bytes as u32)
            .map_err(|_| NodeQueueError::ByteBudgetExhausted)
    }

    fn try_send(&self, command: CoreCommand, bytes: usize) -> Result<(), NodeQueueError> {
        let permit = self.try_acquire_bytes(bytes)?;
        let envelope = CoreEnvelope {
            command,
            _bytes: permit,
        };
        match self.tx.try_send(envelope) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => Err(NodeQueueError::QueueFull),
            Err(mpsc::error::TrySendError::Closed(_)) => Err(NodeQueueError::Closed),
        }
    }

    pub(crate) async fn submit_headers(
        &self,
        headers: Vec<BlockHeader>,
    ) -> Result<Vec<Result<HeaderImportOutcome, ChainStateError>>, NodeQueueError> {
        if headers.len() > HEADER_VALIDATION_SLICE {
            return Err(NodeQueueError::HeaderBatchTooLarge);
        }
        let bytes = headers.iter().map(|header| header.encode().len()).sum();
        let (response, receiver) = oneshot::channel();
        self.try_send(CoreCommand::Headers { headers, response }, bytes)?;
        receiver.await.map_err(|_| NodeQueueError::Closed)
    }

    pub(crate) async fn submit_block(
        &self,
        block: Block,
    ) -> Result<Result<AcceptOutcome, ChainStateError>, NodeQueueError> {
        let bytes = block.encode().len();
        let (response, receiver) = oneshot::channel();
        self.try_send(CoreCommand::Block { block, response }, bytes)?;
        receiver.await.map_err(|_| NodeQueueError::Closed)
    }

    pub(crate) async fn submit_transaction(
        &self,
        transaction: Transaction,
    ) -> Result<Result<AdmissionOutcome, NodeTransactionError>, NodeQueueError> {
        let bytes = transaction.encode().len();
        let (response, receiver) = oneshot::channel();
        self.try_send(
            CoreCommand::Transaction {
                transaction,
                response,
            },
            bytes,
        )?;
        receiver.await.map_err(|_| NodeQueueError::Closed)
    }

    pub(crate) async fn read_active_tip(&self) -> Result<(Hash256, u64), NodeQueueError> {
        let (response, receiver) = oneshot::channel();
        self.try_send(CoreCommand::ActiveTip { response }, CORE_READ_COMMAND_BYTES)?;
        receiver.await.map_err(|_| NodeQueueError::Closed)
    }

    pub(crate) async fn read_preferred_header_tip(
        &self,
    ) -> Result<(Hash256, u64), NodeQueueError> {
        let (response, receiver) = oneshot::channel();
        self.try_send(
            CoreCommand::PreferredHeaderTip { response },
            CORE_READ_COMMAND_BYTES,
        )?;
        receiver.await.map_err(|_| NodeQueueError::Closed)
    }

    pub(crate) async fn read_active_id_at_height(
        &self,
        height: u64,
    ) -> Result<Result<Option<Hash256>, ChainStateError>, NodeQueueError> {
        let (response, receiver) = oneshot::channel();
        self.try_send(
            CoreCommand::ActiveIdAtHeight { height, response },
            CORE_READ_COMMAND_BYTES,
        )?;
        receiver.await.map_err(|_| NodeQueueError::Closed)
    }

    pub(crate) async fn read_preferred_header_id_at_height(
        &self,
        height: u64,
    ) -> Result<Result<Option<Hash256>, ChainStateError>, NodeQueueError> {
        let (response, receiver) = oneshot::channel();
        self.try_send(
            CoreCommand::PreferredHeaderIdAtHeight { height, response },
            CORE_READ_COMMAND_BYTES,
        )?;
        receiver.await.map_err(|_| NodeQueueError::Closed)
    }

    pub(crate) async fn read_preferred_header_at_height(
        &self,
        height: u64,
    ) -> Result<Result<Option<BlockHeader>, ChainStateError>, NodeQueueError> {
        let (response, receiver) = oneshot::channel();
        self.try_send(
            CoreCommand::PreferredHeaderAtHeight { height, response },
            CORE_READ_COMMAND_BYTES,
        )?;
        receiver.await.map_err(|_| NodeQueueError::Closed)
    }

    pub(crate) async fn read_body_retained(
        &self,
        block_id: Hash256,
    ) -> Result<Result<bool, ChainStateError>, NodeQueueError> {
        let (response, receiver) = oneshot::channel();
        self.try_send(
            CoreCommand::BodyRetained { block_id, response },
            CORE_READ_COMMAND_BYTES,
        )?;
        receiver.await.map_err(|_| NodeQueueError::Closed)
    }

    #[cfg(test)]
    pub(crate) fn try_send_headers(&self, headers: Vec<BlockHeader>) -> Result<(), NodeQueueError> {
        if headers.len() > HEADER_VALIDATION_SLICE {
            return Err(NodeQueueError::HeaderBatchTooLarge);
        }
        let bytes = headers.iter().map(|header| header.encode().len()).sum();
        let (response, _receiver) = oneshot::channel();
        self.try_send(CoreCommand::Headers { headers, response }, bytes)
    }

    #[cfg(test)]
    pub(crate) fn try_send_test_bytes(&self, bytes: usize) -> Result<(), NodeQueueError> {
        self.try_send(CoreCommand::TestBytes, bytes)
    }

    #[cfg(test)]
    pub(crate) fn available_bytes(&self) -> usize {
        self.bytes.available_permits()
    }

    #[cfg(test)]
    pub(crate) async fn probe_thread_id(&self) -> Result<std::thread::ThreadId, NodeQueueError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.try_send(CoreCommand::ProbeThread(response_tx), 1)?;
        response_rx.await.map_err(|_| NodeQueueError::Closed)
    }
}

pub(crate) fn spawn_core<V>(
    state: ChainState,
    saved_config: MempoolConfig,
    verifier: V,
) -> Result<CoreHandle, MempoolError>
where
    V: SpendVerifier + Send + 'static,
{
    let base = chain_base(&state);
    let mempool = Mempool::new(base, saved_config.clone())?;
    let (handle, mut receiver) = core_channel();

    drop(tokio::task::spawn_blocking(move || {
        run_core(state, mempool, saved_config, verifier, &mut receiver);
    }));

    Ok(handle)
}

fn run_core<V>(
    mut state: ChainState,
    mut mempool: Mempool,
    saved_config: MempoolConfig,
    verifier: V,
    receiver: &mut mpsc::Receiver<CoreEnvelope>,
) where
    V: SpendVerifier,
{
    while let Some(envelope) = receiver.blocking_recv() {
        match envelope.command {
            CoreCommand::Headers { headers, response } => {
                let results = headers
                    .into_iter()
                    .map(|header| state.accept_header(header))
                    .collect();
                let _ = response.send(results);
            }
            CoreCommand::Block { block, response } => {
                let accepted_block = block.clone();
                let result = state.accept_block(block, &verifier);
                if let Ok(outcome) = &result {
                    let new_base = chain_base(&state);
                    reconcile_after_acceptance(
                        &mut mempool,
                        &saved_config,
                        *outcome,
                        &accepted_block,
                        new_base,
                        state.utxos(),
                        &verifier,
                    );
                }
                let _ = response.send(result);
            }
            CoreCommand::Transaction {
                transaction,
                response,
            } => {
                let result = match state.session_health() {
                    SessionHealth::Healthy => mempool
                        .admit(transaction, chain_base(&state), state.utxos(), &verifier)
                        .map_err(NodeTransactionError::Mempool),
                    health => Err(NodeTransactionError::Unavailable(health)),
                };
                let _ = response.send(result);
            }
            CoreCommand::ActiveTip { response } => {
                let tip = state.tip();
                let _ = response.send((tip.block_id, tip.height));
            }
            CoreCommand::PreferredHeaderTip { response } => {
                let tip = state.preferred_header_tip();
                let _ = response.send((tip.block_id, tip.height));
            }
            CoreCommand::ActiveIdAtHeight { height, response } => {
                let _ = response.send(state.active_id_at_height(height));
            }
            CoreCommand::PreferredHeaderIdAtHeight { height, response } => {
                let _ = response.send(state.preferred_header_id_at_height(height));
            }
            CoreCommand::PreferredHeaderAtHeight { height, response } => {
                let _ = response.send(state.preferred_header_at_height(height));
            }
            CoreCommand::BodyRetained { block_id, response } => {
                let _ = response.send(state.body_retained(block_id));
            }
            #[cfg(test)]
            CoreCommand::TestBytes => {}
            #[cfg(test)]
            CoreCommand::ProbeThread(response) => {
                let _ = response.send(std::thread::current().id());
            }
        }
    }
}

fn chain_base(state: &ChainState) -> ChainBase {
    ChainBase {
        tip_id: state.tip().block_id,
        tip_height: state.tip().height,
    }
}

fn core_channel() -> (CoreHandle, mpsc::Receiver<CoreEnvelope>) {
    let (tx, receiver) = mpsc::channel(MAX_CORE_COMMANDS);
    let bytes = Arc::new(Semaphore::new(MAX_CORE_COMMAND_BYTES));
    (CoreHandle { tx, bytes }, receiver)
}

#[cfg(test)]
pub(crate) fn test_core_channel() -> (CoreHandle, mpsc::Receiver<CoreEnvelope>) {
    core_channel()
}

#[cfg(test)]
pub(crate) struct SyncProbeState {
    pub(crate) active: SyncTip,
    pub(crate) preferred: SyncTip,
    pub(crate) active_at_height: (u64, Hash256),
    pub(crate) preferred_at_height: (u64, Hash256),
    pub(crate) preferred_header: (u64, BlockHeader),
    pub(crate) retained: Hash256,
}

#[cfg(test)]
pub(crate) fn spawn_sync_probe_worker(probe: SyncProbeState) -> CoreHandle {
    let (handle, mut receiver) = test_core_channel();
    drop(tokio::task::spawn_blocking(move || {
        while let Some(envelope) = receiver.blocking_recv() {
            match envelope.command {
                CoreCommand::ActiveTip { response } => {
                    let _ = response.send((probe.active.block_id, probe.active.height));
                }
                CoreCommand::PreferredHeaderTip { response } => {
                    let _ = response.send((probe.preferred.block_id, probe.preferred.height));
                }
                CoreCommand::ActiveIdAtHeight { height, response } => {
                    let value = (height == probe.active_at_height.0)
                        .then_some(probe.active_at_height.1);
                    let _ = response.send(Ok(value));
                }
                CoreCommand::PreferredHeaderIdAtHeight { height, response } => {
                    let value = (height == probe.preferred_at_height.0)
                        .then_some(probe.preferred_at_height.1);
                    let _ = response.send(Ok(value));
                }
                CoreCommand::PreferredHeaderAtHeight { height, response } => {
                    let value = (height == probe.preferred_header.0)
                        .then(|| probe.preferred_header.1.clone());
                    let _ = response.send(Ok(value));
                }
                CoreCommand::BodyRetained { block_id, response } => {
                    let _ = response.send(Ok(block_id == probe.retained));
                }
                CoreCommand::ProbeThread(response) => {
                    let _ = response.send(std::thread::current().id());
                }
                CoreCommand::Headers { headers, .. } => drop(headers),
                CoreCommand::Block { block, .. } => drop(block),
                CoreCommand::Transaction { transaction, .. } => drop(transaction),
                CoreCommand::TestBytes => {}
            }
        }
    }));
    handle
}

#[cfg(test)]
pub(crate) fn spawn_probe_worker() -> CoreHandle {
    let (handle, mut receiver) = test_core_channel();
    drop(tokio::task::spawn_blocking(move || {
        while let Some(envelope) = receiver.blocking_recv() {
            match envelope.command {
                CoreCommand::ProbeThread(response) => {
                    let _ = response.send(std::thread::current().id());
                }
                CoreCommand::Headers { headers, .. } => drop(headers),
                CoreCommand::Block { block, .. } => drop(block),
                CoreCommand::Transaction { transaction, .. } => drop(transaction),
                CoreCommand::ActiveTip { .. }
                | CoreCommand::PreferredHeaderTip { .. }
                | CoreCommand::ActiveIdAtHeight { .. }
                | CoreCommand::PreferredHeaderIdAtHeight { .. }
                | CoreCommand::PreferredHeaderAtHeight { .. }
                | CoreCommand::BodyRetained { .. }
                | CoreCommand::TestBytes => {}
            }
        }
    }));
    handle
}
