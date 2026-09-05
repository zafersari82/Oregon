use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use oregon_protocol::InventoryItem;
use thiserror::Error;
use tokio::time::Instant;

pub const RESPONSE_START_TIMEOUT: Duration = Duration::from_secs(20);
pub const EXPIRED_REQUEST_GRACE: Duration = Duration::from_secs(30);
pub const MAX_RECENTLY_EXPIRED_REQUESTS_PER_PEER: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestKey {
    Headers,
    Object(InventoryItem),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RequestError {
    #[error("request is already outstanding: {0:?}")]
    AlreadyOutstanding(RequestKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseDisposition {
    Matched(RequestKey),
    GraceDrop(RequestKey),
    Unsolicited(RequestKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PerformanceSnapshot {
    pub success_count: u64,
    pub timeout_count: u64,
    pub average_response_latency_ms: u64,
}

#[derive(Debug, Clone, Copy)]
struct OutstandingRequest {
    started_at: Instant,
    generation: u64,
}

#[derive(Debug, Clone, Copy)]
struct GraceEntry {
    key: RequestKey,
    expires_at: Instant,
    generation: u64,
}

#[derive(Debug, Default)]
pub struct RequestRegistry {
    outstanding: HashMap<RequestKey, OutstandingRequest>,
    grace: VecDeque<GraceEntry>,
    next_generation: u64,
    performance: PerformanceSnapshot,
    total_response_latency_ms: u128,
}

impl RequestRegistry {
    pub fn expect(&mut self, key: RequestKey) -> Result<(), RequestError> {
        self.expect_at(key, Instant::now())
    }

    pub fn expect_at(&mut self, key: RequestKey, now: Instant) -> Result<(), RequestError> {
        if self.outstanding.contains_key(&key) {
            return Err(RequestError::AlreadyOutstanding(key));
        }
        self.grace.retain(|entry| entry.key != key);
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1);
        self.outstanding.insert(
            key,
            OutstandingRequest {
                started_at: now,
                generation,
            },
        );
        Ok(())
    }

    pub fn classify_key(&mut self, key: RequestKey) -> ResponseDisposition {
        self.classify_key_at(key, Instant::now())
    }

    pub fn classify_key_at(&mut self, key: RequestKey, now: Instant) -> ResponseDisposition {
        self.prune_grace(now);
        if let Some(request) = self.outstanding.remove(&key) {
            self.record_success(now.saturating_duration_since(request.started_at));
            return ResponseDisposition::Matched(key);
        }
        if let Some(index) = self.grace.iter().position(|entry| entry.key == key) {
            self.grace.remove(index);
            return ResponseDisposition::GraceDrop(key);
        }
        ResponseDisposition::Unsolicited(key)
    }

    pub fn expire(&mut self) -> Vec<RequestKey> {
        self.expire_at(Instant::now())
    }

    pub fn expire_at(&mut self, now: Instant) -> Vec<RequestKey> {
        self.prune_grace(now);
        let mut expired: Vec<_> = self
            .outstanding
            .iter()
            .filter_map(|(key, request)| {
                let deadline = request.started_at + RESPONSE_START_TIMEOUT;
                (now >= deadline).then_some((*key, deadline, request.generation))
            })
            .collect();
        expired.sort_by_key(|(_, deadline, generation)| (*deadline, *generation));

        let mut keys = Vec::with_capacity(expired.len());
        for (key, deadline, generation) in expired {
            self.outstanding.remove(&key);
            self.performance.timeout_count = self.performance.timeout_count.saturating_add(1);
            let grace_expires = deadline + EXPIRED_REQUEST_GRACE;
            if now < grace_expires {
                self.insert_grace(GraceEntry {
                    key,
                    expires_at: grace_expires,
                    generation,
                });
            }
            keys.push(key);
        }
        keys
    }

    pub fn performance(&self) -> PerformanceSnapshot {
        self.performance
    }

    pub fn grace_len(&self) -> usize {
        self.grace.len()
    }

    pub fn outstanding_len(&self) -> usize {
        self.outstanding.len()
    }

    fn record_success(&mut self, latency: Duration) {
        self.performance.success_count = self.performance.success_count.saturating_add(1);
        self.total_response_latency_ms = self
            .total_response_latency_ms
            .saturating_add(latency.as_millis());
        let average = self.total_response_latency_ms / u128::from(self.performance.success_count);
        self.performance.average_response_latency_ms = average.min(u128::from(u64::MAX)) as u64;
    }

    fn insert_grace(&mut self, entry: GraceEntry) {
        self.grace.retain(|existing| existing.key != entry.key);
        if self.grace.len() == MAX_RECENTLY_EXPIRED_REQUESTS_PER_PEER {
            let earliest = self
                .grace
                .iter()
                .enumerate()
                .min_by_key(|(_, existing)| (existing.expires_at, existing.generation))
                .map(|(index, _)| index)
                .expect("non-empty grace set at exact cap");
            self.grace.remove(earliest);
        }
        self.grace.push_back(entry);
    }

    fn prune_grace(&mut self, now: Instant) {
        self.grace.retain(|entry| now < entry.expires_at);
    }
}
