use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use async_trait::async_trait;
use oregon_peer::{PeerId, PerformanceSnapshot};
use oregon_primitives::{Block, BlockHeader, Hash256};

use crate::{
    BlockScheduler, ChainSyncView, MAX_BLOCK_ATTEMPTS, MAX_BUFFERED_BLOCKS,
    MAX_IN_FLIGHT_BLOCKS_GLOBAL, MAX_IN_FLIGHT_BLOCKS_PEER, SyncAction, SyncPeer, SyncTip,
    SyncViewError, build_locator, highest_locator_hit, locator_heights, validate_headers_response,
};

fn hash_for_height(height: u64) -> Hash256 {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&height.to_le_bytes());
    Hash256::from_bytes(bytes)
}

fn header(previous_block: Hash256, nonce: u64) -> BlockHeader {
    BlockHeader {
        version: 1,
        previous_block,
        transaction_root: hash_for_height(nonce.wrapping_add(10_000)),
        timestamp: 1_700_000_000 + nonce,
        difficulty_commitment: [0xff; 32],
        nonce,
    }
}

fn chain_blocks(count: usize) -> Vec<Block> {
    let mut previous = Hash256::from_bytes([0u8; 32]);
    let mut blocks = Vec::with_capacity(count);
    for nonce in 1..=count as u64 {
        let h = header(previous, nonce);
        previous = h.block_id();
        blocks.push(Block {
            header: h,
            transactions: Vec::new(),
        });
    }
    blocks
}

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

struct FakeView {
    active: SyncTip,
    preferred: SyncTip,
}

impl FakeView {
    fn new(active_height: u64, preferred_height: u64) -> Self {
        Self {
            active: SyncTip {
                block_id: hash_for_height(active_height),
                height: active_height,
            },
            preferred: SyncTip {
                block_id: hash_for_height(preferred_height),
                height: preferred_height,
            },
        }
    }
}

#[async_trait]
impl ChainSyncView for FakeView {
    async fn active_tip(&self) -> Result<SyncTip, SyncViewError> {
        Ok(self.active)
    }

    async fn preferred_header_tip(&self) -> Result<SyncTip, SyncViewError> {
        Ok(self.preferred)
    }

    async fn active_id_at_height(&self, height: u64) -> Result<Option<Hash256>, SyncViewError> {
        Ok((height <= self.active.height).then(|| hash_for_height(height)))
    }

    async fn preferred_header_id_at_height(
        &self,
        height: u64,
    ) -> Result<Option<Hash256>, SyncViewError> {
        Ok((height <= self.preferred.height).then(|| hash_for_height(height)))
    }

    async fn preferred_header_at_height(
        &self,
        height: u64,
    ) -> Result<Option<BlockHeader>, SyncViewError> {
        if height == 0 || height > self.preferred.height {
            return Ok(None);
        }
        Ok(Some(header(hash_for_height(height - 1), height)))
    }

    async fn body_retained(&self, block_id: Hash256) -> Result<bool, SyncViewError> {
        Ok(block_id == self.active.block_id)
    }
}

fn peer(id: u64, timeouts: u64, latency_ms: u64, sync_eligible: bool) -> SyncPeer {
    SyncPeer {
        peer_id: PeerId(id),
        block_relay: true,
        sync_eligible,
        performance: PerformanceSnapshot {
            success_count: 1,
            timeout_count: timeouts,
            average_response_latency_ms: latency_ms,
        },
    }
}

#[test]
fn locator_uses_ten_linear_entries_then_exponential_steps_and_anchor() {
    let heights = locator_heights(1_000);
    assert_eq!(
        &heights[..10],
        &[1000, 999, 998, 997, 996, 995, 994, 993, 992, 991]
    );
    assert_eq!(&heights[10..13], &[989, 985, 977]);
    assert_eq!(heights.last(), Some(&0));
    assert!(heights.len() <= 64);

    let huge = locator_heights(u64::MAX - 1);
    assert_eq!(huge.last(), Some(&0));
    assert!(huge.len() <= 64);
}

#[test]
fn locator_builder_reads_only_preferred_validated_header_path() {
    let view = FakeView::new(12, 1_000);
    let request = block_on(build_locator(&view, None)).unwrap();
    let heights = locator_heights(1_000);
    let expected: Vec<_> = heights.into_iter().map(hash_for_height).collect();
    assert_eq!(request.locator, expected);
    assert_eq!(request.stop, None);
}

#[test]
fn highest_locator_hit_prefers_highest_authoritative_path_height() {
    let unknown = Hash256::from_bytes([0xee; 32]);
    let h99 = hash_for_height(99);
    let h90 = hash_for_height(90);
    let local = vec![
        (0, hash_for_height(0)),
        (90, h90),
        (99, h99),
        (100, hash_for_height(100)),
    ];
    assert_eq!(
        highest_locator_hit(&[unknown, h90, h99], &local),
        Some((99, h99))
    );
}

#[test]
fn headers_response_must_attach_and_remain_contiguous() {
    let common = hash_for_height(0);
    let h1 = header(common, 1);
    let h2 = header(h1.block_id(), 2);
    assert!(validate_headers_response(common, &[h1.clone(), h2.clone()]).is_ok());

    let detached = header(hash_for_height(777), 3);
    assert!(validate_headers_response(common, &[detached]).is_err());

    let broken_second = header(hash_for_height(555), 4);
    assert!(validate_headers_response(common, &[h1, broken_second]).is_err());
}

#[test]
fn scheduler_enforces_exact_global_peer_buffer_and_attempt_constants() {
    assert_eq!(MAX_IN_FLIGHT_BLOCKS_GLOBAL, 32);
    assert_eq!(MAX_IN_FLIGHT_BLOCKS_PEER, 8);
    assert_eq!(MAX_BUFFERED_BLOCKS, 32);
    assert_eq!(MAX_BLOCK_ATTEMPTS, 3);

    let blocks = chain_blocks(40);
    let targets: Vec<_> = blocks.iter().map(|block| block.header.block_id()).collect();
    let mut scheduler = BlockScheduler::new(targets).unwrap();
    let peers = vec![
        peer(1, 0, 10, true),
        peer(2, 0, 10, true),
        peer(3, 0, 10, true),
        peer(4, 0, 10, true),
        peer(5, 0, 10, true),
    ];
    let actions = scheduler.schedule(&peers);
    let requests: Vec<_> = actions
        .iter()
        .filter_map(|action| match action {
            SyncAction::RequestBlock { peer_id, block_id } => Some((*peer_id, *block_id)),
            _ => None,
        })
        .collect();
    assert_eq!(requests.len(), 32);
    assert_eq!(scheduler.in_flight_len(), 32);
    for candidate in &peers {
        assert!(scheduler.in_flight_for_peer(candidate.peer_id) <= 8);
    }
}

#[test]
fn scheduler_excludes_ineligible_peer_and_prefers_timeout_latency_then_peer_id() {
    let blocks = chain_blocks(2);
    let targets: Vec<_> = blocks.iter().map(|block| block.header.block_id()).collect();
    let mut scheduler = BlockScheduler::new(targets).unwrap();
    let peers = vec![
        peer(1, 0, 50, false),
        peer(9, 1, 1, true),
        peer(4, 0, 30, true),
        peer(3, 0, 30, true),
    ];
    let actions = scheduler.schedule(&peers);
    let assigned: Vec<_> = actions
        .iter()
        .filter_map(|action| match action {
            SyncAction::RequestBlock { peer_id, .. } => Some(*peer_id),
            _ => None,
        })
        .collect();
    assert_eq!(assigned, vec![PeerId(3), PeerId(3)]);
}

#[test]
fn timeout_releases_ownership_and_third_failed_attempt_stalls_target() {
    let block = chain_blocks(1).remove(0);
    let block_id = block.header.block_id();
    let mut scheduler = BlockScheduler::new(vec![block_id]).unwrap();

    let first = scheduler.schedule(&[peer(1, 0, 10, true), peer(2, 0, 20, true)]);
    assert!(matches!(
        first.as_slice(),
        [SyncAction::RequestBlock {
            peer_id: PeerId(1),
            ..
        }]
    ));
    assert!(scheduler.on_timeout(PeerId(1), block_id).is_empty());
    assert_eq!(scheduler.in_flight_len(), 0);

    let second = scheduler.schedule(&[peer(1, 1, 10, true), peer(2, 0, 20, true)]);
    assert!(matches!(
        second.as_slice(),
        [SyncAction::RequestBlock {
            peer_id: PeerId(2),
            ..
        }]
    ));
    assert!(scheduler.on_timeout(PeerId(2), block_id).is_empty());

    let third = scheduler.schedule(&[peer(1, 1, 10, true), peer(2, 1, 20, true)]);
    assert!(matches!(
        third.as_slice(),
        [SyncAction::RequestBlock {
            peer_id: PeerId(1),
            ..
        }]
    ));
    assert_eq!(
        scheduler.on_timeout(PeerId(1), block_id),
        vec![SyncAction::Stalled { block_id }]
    );
    assert_eq!(scheduler.attempts(block_id), 3);
}

#[test]
fn out_of_order_blocks_are_buffered_but_submitted_in_preferred_path_order() {
    let blocks = chain_blocks(3);
    let targets: Vec<_> = blocks.iter().map(|block| block.header.block_id()).collect();
    let mut scheduler = BlockScheduler::new(targets).unwrap();
    scheduler.schedule(&[peer(1, 0, 10, true)]);

    let middle = scheduler.on_block(PeerId(1), blocks[1].clone()).unwrap();
    assert!(middle.is_empty());
    assert_eq!(scheduler.buffered_len(), 1);

    let first = scheduler.on_block(PeerId(1), blocks[0].clone()).unwrap();
    assert_eq!(
        first,
        vec![
            SyncAction::SubmitBlock {
                source: PeerId(1),
                block: blocks[0].clone(),
            },
            SyncAction::SubmitBlock {
                source: PeerId(1),
                block: blocks[1].clone(),
            },
        ]
    );
    assert_eq!(scheduler.buffered_len(), 0);

    let last = scheduler.on_block(PeerId(1), blocks[2].clone()).unwrap();
    assert_eq!(
        last,
        vec![SyncAction::SubmitBlock {
            source: PeerId(1),
            block: blocks[2].clone(),
        }]
    );
}

#[test]
fn sync_layer_has_no_chainwork_or_remote_height_authority() {
    let source = concat!(
        include_str!("locator.rs"),
        include_str!("scheduler.rs"),
        include_str!("state.rs"),
        include_str!("view.rs")
    );
    assert!(!source.contains("ChainWork"));
    assert!(!source.contains("remote_best_height"));
}
