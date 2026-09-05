use std::collections::{HashMap, HashSet};

use oregon_peer::{PeerId, PerformanceSnapshot};
use oregon_primitives::{Block, Hash256};

use crate::{SyncAction, SyncError};

pub const MAX_IN_FLIGHT_BLOCKS_GLOBAL: usize = 32;
pub const MAX_IN_FLIGHT_BLOCKS_PEER: usize = 8;
pub const MAX_BUFFERED_BLOCKS: usize = 32;
pub const MAX_BLOCK_ATTEMPTS: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncPeer {
    pub peer_id: PeerId,
    pub block_relay: bool,
    pub sync_eligible: bool,
    pub performance: PerformanceSnapshot,
}

#[derive(Debug, Clone, Copy)]
struct InFlight {
    peer_id: PeerId,
}

#[derive(Debug, Clone)]
struct BufferedBlock {
    source: PeerId,
    block: Block,
}

#[derive(Debug)]
pub struct BlockScheduler {
    targets: Vec<Hash256>,
    target_index: HashMap<Hash256, usize>,
    next_submit: usize,
    in_flight: HashMap<Hash256, InFlight>,
    in_flight_per_peer: HashMap<PeerId, usize>,
    attempts: HashMap<Hash256, u8>,
    buffered: HashMap<Hash256, BufferedBlock>,
    stalled: HashSet<Hash256>,
}

impl BlockScheduler {
    pub fn new(targets: Vec<Hash256>) -> Result<Self, SyncError> {
        let mut target_index = HashMap::with_capacity(targets.len());
        for (index, block_id) in targets.iter().copied().enumerate() {
            if target_index.insert(block_id, index).is_some() {
                return Err(SyncError::DuplicateTarget(block_id));
            }
        }
        Ok(Self {
            targets,
            target_index,
            next_submit: 0,
            in_flight: HashMap::new(),
            in_flight_per_peer: HashMap::new(),
            attempts: HashMap::new(),
            buffered: HashMap::new(),
            stalled: HashSet::new(),
        })
    }

    pub fn schedule(&mut self, peers: &[SyncPeer]) -> Vec<SyncAction> {
        let mut eligible: Vec<_> = peers
            .iter()
            .copied()
            .filter(|peer| peer.block_relay && peer.sync_eligible)
            .collect();
        eligible.sort_by_key(|peer| {
            (
                peer.performance.timeout_count,
                peer.performance.average_response_latency_ms,
                peer.peer_id,
            )
        });

        let mut actions = Vec::new();
        if eligible.is_empty() {
            return actions;
        }

        for index in self.next_submit..self.targets.len() {
            if self.in_flight.len() >= MAX_IN_FLIGHT_BLOCKS_GLOBAL {
                break;
            }
            let block_id = self.targets[index];
            if self.in_flight.contains_key(&block_id)
                || self.buffered.contains_key(&block_id)
                || self.stalled.contains(&block_id)
                || self
                    .attempts
                    .get(&block_id)
                    .is_some_and(|attempts| *attempts >= MAX_BLOCK_ATTEMPTS)
            {
                continue;
            }

            let Some(peer) = eligible
                .iter()
                .find(|peer| self.in_flight_for_peer(peer.peer_id) < MAX_IN_FLIGHT_BLOCKS_PEER)
            else {
                break;
            };

            self.in_flight.insert(
                block_id,
                InFlight {
                    peer_id: peer.peer_id,
                },
            );
            *self.in_flight_per_peer.entry(peer.peer_id).or_default() += 1;
            *self.attempts.entry(block_id).or_default() += 1;
            actions.push(SyncAction::RequestBlock {
                peer_id: peer.peer_id,
                block_id,
            });
        }
        actions
    }

    pub fn on_timeout(&mut self, peer_id: PeerId, block_id: Hash256) -> Vec<SyncAction> {
        let Some(owner) = self.in_flight.get(&block_id).copied() else {
            return Vec::new();
        };
        if owner.peer_id != peer_id {
            return Vec::new();
        }
        self.release(block_id, peer_id);
        if self.attempts(block_id) >= MAX_BLOCK_ATTEMPTS && self.stalled.insert(block_id) {
            return vec![SyncAction::Stalled { block_id }];
        }
        Vec::new()
    }

    pub fn on_block(
        &mut self,
        peer_id: PeerId,
        block: Block,
    ) -> Result<Vec<SyncAction>, SyncError> {
        let block_id = block.header.block_id();
        let Some(owner) = self.in_flight.get(&block_id).copied() else {
            return Err(SyncError::UnexpectedBlock { peer_id, block_id });
        };
        if owner.peer_id != peer_id {
            return Err(SyncError::UnexpectedBlock { peer_id, block_id });
        }
        self.release(block_id, peer_id);

        let index = self
            .target_index
            .get(&block_id)
            .copied()
            .ok_or(SyncError::UnexpectedBlock { peer_id, block_id })?;
        if index < self.next_submit {
            return Err(SyncError::UnexpectedBlock { peer_id, block_id });
        }
        if self.buffered.len() >= MAX_BUFFERED_BLOCKS {
            return Err(SyncError::BufferFull);
        }
        self.buffered.insert(
            block_id,
            BufferedBlock {
                source: peer_id,
                block,
            },
        );

        let mut actions = Vec::new();
        while self.next_submit < self.targets.len() {
            let expected = self.targets[self.next_submit];
            let Some(buffered) = self.buffered.remove(&expected) else {
                break;
            };
            actions.push(SyncAction::SubmitBlock {
                source: buffered.source,
                block: buffered.block,
            });
            self.next_submit += 1;
        }
        Ok(actions)
    }

    pub fn in_flight_len(&self) -> usize {
        self.in_flight.len()
    }

    pub fn in_flight_for_peer(&self, peer_id: PeerId) -> usize {
        self.in_flight_per_peer.get(&peer_id).copied().unwrap_or(0)
    }

    pub fn buffered_len(&self) -> usize {
        self.buffered.len()
    }

    pub fn attempts(&self, block_id: Hash256) -> u8 {
        self.attempts.get(&block_id).copied().unwrap_or(0)
    }

    pub fn is_complete(&self) -> bool {
        self.next_submit == self.targets.len()
    }

    fn release(&mut self, block_id: Hash256, peer_id: PeerId) {
        self.in_flight.remove(&block_id);
        if let Some(count) = self.in_flight_per_peer.get_mut(&peer_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.in_flight_per_peer.remove(&peer_id);
            }
        }
    }
}
