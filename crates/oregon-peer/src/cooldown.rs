use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

use tokio::time::Instant;

pub const DISCONNECT_COOLDOWN: Duration = Duration::from_secs(10 * 60);
pub const MAX_COOLDOWN_ENTRIES: usize = 1_024;

#[derive(Debug, Default)]
pub struct CooldownTable {
    entries: HashMap<IpAddr, Instant>,
}

impl CooldownTable {
    pub fn insert(&mut self, ip: IpAddr) {
        self.insert_at(ip, Instant::now());
    }

    pub fn insert_at(&mut self, ip: IpAddr, now: Instant) {
        self.prune(now);
        let ip = canonical_ip(ip);
        let expiry = now + DISCONNECT_COOLDOWN;
        if !self.entries.contains_key(&ip) && self.entries.len() == MAX_COOLDOWN_ENTRIES {
            if let Some(evict) = self
                .entries
                .iter()
                .min_by_key(|(candidate, candidate_expiry)| (**candidate_expiry, **candidate))
                .map(|(candidate, _)| *candidate)
            {
                self.entries.remove(&evict);
            }
        }
        self.entries.insert(ip, expiry);
    }

    pub fn contains(&mut self, ip: IpAddr) -> bool {
        self.contains_at(ip, Instant::now())
    }

    pub fn contains_at(&mut self, ip: IpAddr, now: Instant) -> bool {
        self.prune(now);
        self.entries
            .get(&canonical_ip(ip))
            .is_some_and(|expiry| now < *expiry)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn prune(&mut self, now: Instant) {
        self.entries.retain(|_, expiry| now < *expiry);
    }
}

pub fn canonical_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(ipv6) => ipv6
            .to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(ipv6)),
        ipv4 => ipv4,
    }
}
