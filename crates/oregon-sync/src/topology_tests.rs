use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use async_trait::async_trait;
use oregon_primitives::{BlockHeader, Hash256};

use crate::{
    ChainSyncView, SyncTip, SyncViewError, find_common_height, missing_body_targets,
};

#[derive(Clone, Copy)]
struct ImmediateWake;

impl Wake for ImmediateWake {
    fn wake(self: Arc<Self>) {}
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ImmediateWake));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn id(tag: u8, height: u64) -> Hash256 {
    let mut bytes = [0u8; 32];
    bytes[0] = tag;
    bytes[1..9].copy_from_slice(&height.to_le_bytes());
    Hash256::from_bytes(bytes)
}

struct ForkedView {
    active: BTreeMap<u64, Hash256>,
    preferred: BTreeMap<u64, Hash256>,
    retained: HashSet<Hash256>,
}

impl ForkedView {
    fn fixture() -> Self {
        let mut active = BTreeMap::new();
        let mut preferred = BTreeMap::new();
        for height in 0..=5 {
            let common = id(0, height);
            active.insert(height, common);
            preferred.insert(height, common);
        }
        active.insert(6, id(b'a', 6));
        active.insert(7, id(b'a', 7));
        preferred.insert(6, id(b'b', 6));
        preferred.insert(7, id(b'b', 7));
        preferred.insert(8, id(b'b', 8));
        preferred.insert(9, id(b'b', 9));
        Self {
            active,
            preferred,
            retained: HashSet::from([id(b'b', 7), id(b'b', 9)]),
        }
    }

    fn tip(path: &BTreeMap<u64, Hash256>) -> SyncTip {
        let (&height, &block_id) = path.last_key_value().expect("fixture path is non-empty");
        SyncTip { block_id, height }
    }
}

#[async_trait]
impl ChainSyncView for ForkedView {
    async fn active_tip(&self) -> Result<SyncTip, SyncViewError> {
        Ok(Self::tip(&self.active))
    }

    async fn preferred_header_tip(&self) -> Result<SyncTip, SyncViewError> {
        Ok(Self::tip(&self.preferred))
    }

    async fn active_id_at_height(&self, height: u64) -> Result<Option<Hash256>, SyncViewError> {
        Ok(self.active.get(&height).copied())
    }

    async fn preferred_header_id_at_height(
        &self,
        height: u64,
    ) -> Result<Option<Hash256>, SyncViewError> {
        Ok(self.preferred.get(&height).copied())
    }

    async fn preferred_header_at_height(
        &self,
        _height: u64,
    ) -> Result<Option<BlockHeader>, SyncViewError> {
        Ok(None)
    }

    async fn body_retained(&self, block_id: Hash256) -> Result<bool, SyncViewError> {
        Ok(self.retained.contains(&block_id))
    }
}

#[test]
fn common_height_is_highest_local_active_preferred_id_match() {
    let view = ForkedView::fixture();
    assert_eq!(block_on(find_common_height(&view)).unwrap(), 5);
}

#[test]
fn missing_body_targets_follow_preferred_fork_and_skip_retained_bodies() {
    let view = ForkedView::fixture();
    assert_eq!(
        block_on(missing_body_targets(&view)).unwrap(),
        vec![id(b'b', 6), id(b'b', 8)]
    );
}
