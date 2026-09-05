use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use oregon_network::{Transport, TransportConnection};
use oregon_protocol::{Hello, Message};
use tokio::time::Instant;

use crate::budget::{GlobalQueueBudget, PeerQueueBudget};
use crate::cooldown::CooldownTable;
use crate::handshake::{HandshakeResult, perform_handshake, preferred_direction};
use crate::{
    Direction, EstablishedPeer, PeerConfig, PeerError, PeerFeedback, PeerId, PeerSession,
    QueueClass, RequestKey,
};

pub const PING_INTERVAL: Duration = Duration::from_secs(30);
pub const PONG_TIMEOUT: Duration = Duration::from_secs(15);
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LivenessAction {
    None,
    SendPing(u64),
    Disconnect,
}

#[derive(Debug, Clone, Copy)]
pub struct LivenessState {
    last_activity: Instant,
    outstanding_ping: Option<(u64, Instant)>,
    next_ping_nonce: u64,
}

impl LivenessState {
    pub fn new(now: Instant) -> Self {
        Self {
            last_activity: now,
            outstanding_ping: None,
            next_ping_nonce: 1,
        }
    }

    pub fn note_activity(&mut self, now: Instant) {
        self.last_activity = now;
    }

    pub fn on_pong(&mut self, nonce: u64, now: Instant) -> bool {
        if self
            .outstanding_ping
            .is_some_and(|(expected, _)| expected == nonce)
        {
            self.outstanding_ping = None;
            self.last_activity = now;
            true
        } else {
            false
        }
    }

    pub fn poll_at(&mut self, now: Instant) -> LivenessAction {
        if now.saturating_duration_since(self.last_activity) >= IDLE_TIMEOUT {
            return LivenessAction::Disconnect;
        }
        if let Some((_, sent_at)) = self.outstanding_ping {
            if now.saturating_duration_since(sent_at) >= PONG_TIMEOUT {
                return LivenessAction::Disconnect;
            }
            return LivenessAction::None;
        }
        if now.saturating_duration_since(self.last_activity) >= PING_INTERVAL {
            let nonce = self.next_ping_nonce;
            self.next_ping_nonce = self.next_ping_nonce.wrapping_add(1);
            if self.next_ping_nonce == 0 {
                self.next_ping_nonce = 1;
            }
            self.outstanding_ping = Some((nonce, now));
            return LivenessAction::SendPing(nonce);
        }
        LivenessAction::None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerCommand {
    Send {
        peer_id: PeerId,
        message: Message,
        class: QueueClass,
    },
    Expect {
        peer_id: PeerId,
        key: RequestKey,
    },
    Disconnect {
        peer_id: PeerId,
    },
    Feedback {
        peer_id: PeerId,
        feedback: PeerFeedback,
    },
}

#[derive(Debug, Clone, Copy)]
struct RegisteredPeer {
    peer_id: PeerId,
    direction: Direction,
}

#[derive(Debug)]
struct Registry {
    pending: usize,
    inbound: usize,
    outbound: usize,
    next_peer_id: u64,
    by_nonce: HashMap<[u8; 16], RegisteredPeer>,
}

impl Registry {
    fn new() -> Self {
        Self {
            pending: 0,
            inbound: 0,
            outbound: 0,
            next_peer_id: 1,
            by_nonce: HashMap::new(),
        }
    }

    fn count(&self, direction: Direction) -> usize {
        match direction {
            Direction::Inbound => self.inbound,
            Direction::Outbound => self.outbound,
        }
    }

    fn increment(&mut self, direction: Direction) {
        match direction {
            Direction::Inbound => self.inbound += 1,
            Direction::Outbound => self.outbound += 1,
        }
    }

    fn decrement(&mut self, direction: Direction) {
        match direction {
            Direction::Inbound => self.inbound = self.inbound.saturating_sub(1),
            Direction::Outbound => self.outbound = self.outbound.saturating_sub(1),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PendingHandshakes {
    registry: Arc<Mutex<Registry>>,
}

impl PendingHandshakes {
    #[cfg(test)]
    pub(crate) fn standalone() -> Self {
        Self {
            registry: Arc::new(Mutex::new(Registry::new())),
        }
    }

    pub(crate) fn acquire(&self) -> Result<PendingGuard, PeerError> {
        let mut registry = self.registry.lock().expect("peer registry poisoned");
        if registry.pending >= crate::MAX_PENDING_HANDSHAKES {
            return Err(PeerError::PendingHandshakeLimit);
        }
        registry.pending += 1;
        Ok(PendingGuard {
            registry: self.registry.clone(),
            released: false,
        })
    }
}

#[derive(Debug)]
pub(crate) struct PendingGuard {
    registry: Arc<Mutex<Registry>>,
    released: bool,
}

impl Drop for PendingGuard {
    fn drop(&mut self) {
        if !self.released {
            let mut registry = self.registry.lock().expect("peer registry poisoned");
            registry.pending = registry.pending.saturating_sub(1);
            self.released = true;
        }
    }
}

#[derive(Debug)]
pub(crate) struct Registration {
    registry: Arc<Mutex<Registry>>,
    peer_id: PeerId,
    remote_nonce: [u8; 16],
}

impl Drop for Registration {
    fn drop(&mut self) {
        let mut registry = self.registry.lock().expect("peer registry poisoned");
        let current = registry.by_nonce.get(&self.remote_nonce).copied();
        if current.map(|peer| peer.peer_id) == Some(self.peer_id) {
            if let Some(peer) = registry.by_nonce.remove(&self.remote_nonce) {
                registry.decrement(peer.direction);
            }
        }
    }
}

pub struct EstablishOutcome<C: TransportConnection> {
    pub session: PeerSession<C>,
    pub replaced_peer: Option<PeerId>,
}

pub struct PeerService<T: Transport> {
    transport: T,
    config: PeerConfig,
    local_hello: Hello,
    registry: Arc<Mutex<Registry>>,
    pending: PendingHandshakes,
    global_budget: GlobalQueueBudget,
    cooldown: Mutex<CooldownTable>,
}

impl<T: Transport> PeerService<T> {
    pub fn new(transport: T, config: PeerConfig, local_hello: Hello) -> Result<Self, PeerError> {
        let config = PeerConfig::new(config.max_peers, config.max_outbound, config.max_inbound)?;
        let registry = Arc::new(Mutex::new(Registry::new()));
        Ok(Self {
            transport,
            config,
            local_hello,
            pending: PendingHandshakes {
                registry: registry.clone(),
            },
            registry,
            global_budget: GlobalQueueBudget::new(),
            cooldown: Mutex::new(CooldownTable::default()),
        })
    }

    pub fn local_nonce(&self) -> [u8; 16] {
        self.local_hello.instance_nonce
    }

    pub fn config(&self) -> PeerConfig {
        self.config
    }

    pub fn cooldown_remote(&self, ip: IpAddr) {
        self.cooldown
            .lock()
            .expect("peer cooldown poisoned")
            .insert(ip);
    }

    pub fn is_cooling_down(&self, ip: IpAddr) -> bool {
        self.cooldown
            .lock()
            .expect("peer cooldown poisoned")
            .contains(ip)
    }

    pub async fn connect(
        &self,
        addr: SocketAddr,
        magic: [u8; 4],
    ) -> Result<EstablishOutcome<T::Connection>, PeerError> {
        if self.is_cooling_down(addr.ip()) {
            return Err(PeerError::Cooldown);
        }
        let _pending = self.pending.acquire()?;
        let connection = self.transport.connect(addr, magic).await?;
        self.establish(connection, Direction::Outbound).await
    }

    pub async fn accept_connection(
        &self,
        connection: T::Connection,
    ) -> Result<EstablishOutcome<T::Connection>, PeerError> {
        if self.is_cooling_down(connection.remote_addr().ip()) {
            return Err(PeerError::Cooldown);
        }
        let _pending = self.pending.acquire()?;
        self.establish(connection, Direction::Inbound).await
    }

    async fn establish(
        &self,
        mut connection: T::Connection,
        direction: Direction,
    ) -> Result<EstablishOutcome<T::Connection>, PeerError> {
        let remote_addr = connection.remote_addr();
        let handshake = perform_handshake(&mut connection, self.local_hello.clone()).await?;
        let (established, registration, replaced_peer) =
            self.register(remote_addr, direction, &handshake)?;
        let budget = PeerQueueBudget::new(self.global_budget.clone());
        Ok(EstablishOutcome {
            session: PeerSession::new(established, connection, budget, registration),
            replaced_peer,
        })
    }

    fn register(
        &self,
        remote_addr: SocketAddr,
        direction: Direction,
        handshake: &HandshakeResult,
    ) -> Result<(EstablishedPeer, Registration, Option<PeerId>), PeerError> {
        let remote_nonce = handshake.remote.instance_nonce;
        let preferred = preferred_direction(self.local_hello.instance_nonce, remote_nonce)?;
        let mut registry = self.registry.lock().expect("peer registry poisoned");
        let existing = registry.by_nonce.get(&remote_nonce).copied();
        let replacing = match existing {
            Some(peer) if peer.direction == preferred => return Err(PeerError::DuplicatePeer),
            Some(_) if direction != preferred => return Err(PeerError::DuplicatePeer),
            Some(peer) => Some(peer),
            None => None,
        };

        let direction_limit = match direction {
            Direction::Inbound => self.config.max_inbound,
            Direction::Outbound => self.config.max_outbound,
        };
        if registry.count(direction) >= direction_limit {
            return Err(PeerError::DirectionLimit(direction));
        }
        if replacing.is_none() && registry.by_nonce.len() >= self.config.max_peers {
            return Err(PeerError::PeerLimit);
        }

        let peer_id = PeerId(registry.next_peer_id);
        registry.next_peer_id = registry
            .next_peer_id
            .checked_add(1)
            .ok_or(PeerError::PeerIdExhausted)?;

        let replaced_peer = replacing.map(|peer| {
            registry.decrement(peer.direction);
            peer.peer_id
        });
        registry.increment(direction);
        registry
            .by_nonce
            .insert(remote_nonce, RegisteredPeer { peer_id, direction });

        let established = EstablishedPeer {
            peer_id,
            remote_addr,
            direction,
            negotiated_version: handshake.negotiated.protocol_version,
            features: handshake.negotiated.features,
            remote_best_height: handshake.remote.best_height,
            remote_best_block_id: handshake.remote.best_block_id,
        };
        let registration = Registration {
            registry: self.registry.clone(),
            peer_id,
            remote_nonce,
        };
        Ok((established, registration, replaced_peer))
    }
}
