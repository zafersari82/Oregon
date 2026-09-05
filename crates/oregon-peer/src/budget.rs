use std::sync::{Arc, Mutex};

use tokio::sync::Notify;
use tokio::time::{Instant, timeout_at};

use crate::{
    CONTROL_RESERVED_BYTES, CONTROL_RESERVED_FRAMES, MAX_QUEUE_BYTES_GLOBAL, MAX_QUEUE_BYTES_PEER,
    MAX_QUEUE_FRAMES_PEER, PeerError, QUEUE_ENQUEUE_TIMEOUT, QueueClass,
};

#[derive(Debug, Default)]
struct GlobalState {
    bytes: usize,
}

#[derive(Debug, Default)]
struct PeerState {
    frames: usize,
    bytes: usize,
    non_control_frames: usize,
    non_control_bytes: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct GlobalQueueBudget {
    state: Arc<Mutex<GlobalState>>,
    notify: Arc<Notify>,
}

impl GlobalQueueBudget {
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(GlobalState::default())),
            notify: Arc::new(Notify::new()),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PeerQueueBudget {
    global: GlobalQueueBudget,
    state: Arc<Mutex<PeerState>>,
}

impl PeerQueueBudget {
    pub(crate) fn new(global: GlobalQueueBudget) -> Self {
        Self {
            global,
            state: Arc::new(Mutex::new(PeerState::default())),
        }
    }

    pub(crate) fn try_reserve(
        &self,
        class: QueueClass,
        bytes: usize,
    ) -> Result<Option<QueuePermit>, PeerError> {
        if bytes > MAX_QUEUE_BYTES_PEER || bytes > MAX_QUEUE_BYTES_GLOBAL {
            return if class == QueueClass::Gossip {
                Ok(None)
            } else {
                Err(PeerError::QueueItemTooLarge)
            };
        }

        let mut global = self
            .global
            .state
            .lock()
            .expect("queue global budget poisoned");
        let mut peer = self.state.lock().expect("peer queue budget poisoned");

        let total_fits = peer.frames < MAX_QUEUE_FRAMES_PEER
            && peer.bytes.saturating_add(bytes) <= MAX_QUEUE_BYTES_PEER
            && global.bytes.saturating_add(bytes) <= MAX_QUEUE_BYTES_GLOBAL;
        let class_fits = if class == QueueClass::Control {
            true
        } else {
            peer.non_control_frames < MAX_QUEUE_FRAMES_PEER - CONTROL_RESERVED_FRAMES
                && peer.non_control_bytes.saturating_add(bytes)
                    <= MAX_QUEUE_BYTES_PEER - CONTROL_RESERVED_BYTES
        };

        if !total_fits || !class_fits {
            return Ok(None);
        }

        peer.frames += 1;
        peer.bytes += bytes;
        if class != QueueClass::Control {
            peer.non_control_frames += 1;
            peer.non_control_bytes += bytes;
        }
        global.bytes += bytes;

        Ok(Some(QueuePermit {
            budget: self.clone(),
            class,
            bytes,
            released: false,
        }))
    }

    pub(crate) async fn reserve(
        &self,
        class: QueueClass,
        bytes: usize,
    ) -> Result<Option<QueuePermit>, PeerError> {
        if class == QueueClass::Gossip {
            return self.try_reserve(class, bytes);
        }

        let deadline = Instant::now() + QUEUE_ENQUEUE_TIMEOUT;
        loop {
            let notified = self.global.notify.notified();
            if let Some(permit) = self.try_reserve(class, bytes)? {
                return Ok(Some(permit));
            }
            if timeout_at(deadline, notified).await.is_err() {
                return Err(PeerError::QueueEnqueueTimeout);
            }
        }
    }

    fn release(&self, class: QueueClass, bytes: usize) {
        let mut global = self
            .global
            .state
            .lock()
            .expect("queue global budget poisoned");
        let mut peer = self.state.lock().expect("peer queue budget poisoned");
        peer.frames = peer.frames.saturating_sub(1);
        peer.bytes = peer.bytes.saturating_sub(bytes);
        if class != QueueClass::Control {
            peer.non_control_frames = peer.non_control_frames.saturating_sub(1);
            peer.non_control_bytes = peer.non_control_bytes.saturating_sub(bytes);
        }
        global.bytes = global.bytes.saturating_sub(bytes);
        drop(peer);
        drop(global);
        self.global.notify.notify_waiters();
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> (usize, usize, usize, usize) {
        let peer = self.state.lock().expect("peer queue budget poisoned");
        (
            peer.frames,
            peer.bytes,
            peer.non_control_frames,
            peer.non_control_bytes,
        )
    }
}

#[derive(Debug)]
pub(crate) struct QueuePermit {
    budget: PeerQueueBudget,
    class: QueueClass,
    bytes: usize,
    released: bool,
}

impl Drop for QueuePermit {
    fn drop(&mut self) {
        if !self.released {
            self.budget.release(self.class, self.bytes);
            self.released = true;
        }
    }
}
