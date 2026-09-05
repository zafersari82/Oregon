use std::sync::Arc;

use oregon_primitives::BlockHeader;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};

pub(crate) const MAX_CORE_COMMANDS: usize = 64;
pub(crate) const MAX_CORE_COMMAND_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const HEADER_VALIDATION_SLICE: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoreSendError {
    ByteBudgetExhausted,
    QueueFull,
    Closed,
    HeaderBatchTooLarge,
}

pub(crate) struct CoreHandle {
    pub(crate) tx: mpsc::Sender<CoreEnvelope>,
    pub(crate) bytes: Arc<Semaphore>,
}

pub(crate) struct CoreEnvelope {
    command: CoreCommand,
    _bytes: OwnedSemaphorePermit,
}

enum CoreCommand {
    Headers(Vec<BlockHeader>),
    #[cfg(test)]
    TestBytes,
    #[cfg(test)]
    ProbeThread(tokio::sync::oneshot::Sender<std::thread::ThreadId>),
}

impl CoreHandle {
    fn try_acquire_bytes(&self, bytes: usize) -> Result<OwnedSemaphorePermit, CoreSendError> {
        if bytes > MAX_CORE_COMMAND_BYTES {
            return Err(CoreSendError::ByteBudgetExhausted);
        }
        Arc::clone(&self.bytes)
            .try_acquire_many_owned(bytes as u32)
            .map_err(|_| CoreSendError::ByteBudgetExhausted)
    }

    fn try_send(&self, command: CoreCommand, bytes: usize) -> Result<(), CoreSendError> {
        let permit = self.try_acquire_bytes(bytes)?;
        let envelope = CoreEnvelope {
            command,
            _bytes: permit,
        };
        match self.tx.try_send(envelope) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => Err(CoreSendError::QueueFull),
            Err(mpsc::error::TrySendError::Closed(_)) => Err(CoreSendError::Closed),
        }
    }

    pub(crate) fn try_send_headers(&self, headers: Vec<BlockHeader>) -> Result<(), CoreSendError> {
        if headers.len() > HEADER_VALIDATION_SLICE {
            return Err(CoreSendError::HeaderBatchTooLarge);
        }
        let bytes = headers.iter().map(|header| header.encode().len()).sum();
        self.try_send(CoreCommand::Headers(headers), bytes)
    }

    #[cfg(test)]
    pub(crate) fn try_send_test_bytes(&self, bytes: usize) -> Result<(), CoreSendError> {
        self.try_send(CoreCommand::TestBytes, bytes)
    }

    #[cfg(test)]
    pub(crate) fn available_bytes(&self) -> usize {
        self.bytes.available_permits()
    }

    #[cfg(test)]
    pub(crate) async fn probe_thread_id(&self) -> Result<std::thread::ThreadId, CoreSendError> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.try_send(CoreCommand::ProbeThread(response_tx), 1)?;
        response_rx.await.map_err(|_| CoreSendError::Closed)
    }
}

#[cfg(test)]
pub(crate) fn test_core_channel() -> (CoreHandle, mpsc::Receiver<CoreEnvelope>) {
    let (tx, receiver) = mpsc::channel(MAX_CORE_COMMANDS);
    let bytes = Arc::new(Semaphore::new(MAX_CORE_COMMAND_BYTES));
    (CoreHandle { tx, bytes }, receiver)
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
                CoreCommand::Headers(headers) => drop(headers),
                CoreCommand::TestBytes => {}
            }
        }
    }));
    handle
}
