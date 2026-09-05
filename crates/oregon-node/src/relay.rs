use std::collections::{HashMap, HashSet, VecDeque};

use oregon_peer::{PeerCommand, PeerId, QueueClass, RequestKey};
use oregon_primitives::Hash256;
use oregon_protocol::{InventoryItem, InventoryKind, Message};

pub(crate) const MAX_KNOWN_INVENTORY_PER_PEER: usize = 8_192;
pub(crate) const MAX_RECENT_RELAY_CACHE: usize = 65_536;

#[derive(Debug, PartialEq, Eq)]
pub struct ValidatedRelay {
    item: InventoryItem,
}

impl ValidatedRelay {
    pub fn inventory(&self) -> InventoryItem {
        self.item
    }
}

struct BoundedInventory {
    capacity: usize,
    members: HashSet<InventoryItem>,
    order: VecDeque<InventoryItem>,
}

impl BoundedInventory {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            members: HashSet::new(),
            order: VecDeque::new(),
        }
    }

    fn insert(&mut self, item: InventoryItem) -> bool {
        if self.members.contains(&item) {
            return false;
        }
        if self.order.len() == self.capacity {
            let oldest = self
                .order
                .pop_front()
                .expect("bounded inventory is non-empty at capacity");
            let removed = self.members.remove(&oldest);
            debug_assert!(removed);
        }
        let inserted = self.members.insert(item);
        debug_assert!(inserted);
        self.order.push_back(item);
        true
    }

    fn contains(&self, item: InventoryItem) -> bool {
        self.members.contains(&item)
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.order.len()
    }
}

pub(crate) struct RelayState {
    known_by_peer: HashMap<PeerId, BoundedInventory>,
    recent_relay: BoundedInventory,
}

impl Default for RelayState {
    fn default() -> Self {
        Self {
            known_by_peer: HashMap::new(),
            recent_relay: BoundedInventory::new(MAX_RECENT_RELAY_CACHE),
        }
    }
}

impl RelayState {
    pub(crate) fn note_peer_inventory(&mut self, peer_id: PeerId, item: InventoryItem) -> bool {
        self.known_by_peer
            .entry(peer_id)
            .or_insert_with(|| BoundedInventory::new(MAX_KNOWN_INVENTORY_PER_PEER))
            .insert(item)
    }

    pub(crate) fn peer_knows(&self, peer_id: PeerId, item: InventoryItem) -> bool {
        self.known_by_peer
            .get(&peer_id)
            .is_some_and(|known| known.contains(item))
    }

    pub(crate) fn note_recent_relay(&mut self, item: InventoryItem) -> bool {
        self.recent_relay.insert(item)
    }

    pub(crate) fn relay_inventory<I>(
        &mut self,
        source_peer: Option<PeerId>,
        peers: I,
        authorization: &ValidatedRelay,
    ) -> Vec<PeerCommand>
    where
        I: IntoIterator<Item = PeerId>,
    {
        let item = authorization.inventory();
        if let Some(source_peer) = source_peer {
            self.note_peer_inventory(source_peer, item);
        }
        if !self.note_recent_relay(item) {
            return Vec::new();
        }

        let mut peers: Vec<_> = peers.into_iter().collect();
        peers.sort_unstable();
        peers.dedup();

        let mut commands = Vec::new();
        for peer_id in peers {
            if Some(peer_id) == source_peer || self.peer_knows(peer_id, item) {
                continue;
            }
            self.note_peer_inventory(peer_id, item);
            commands.push(PeerCommand::Send {
                peer_id,
                message: Message::Inv(vec![item]),
                class: QueueClass::Gossip,
            });
        }
        commands
    }

    #[cfg(test)]
    pub(crate) fn known_inventory_len(&self, peer_id: PeerId) -> usize {
        self.known_by_peer
            .get(&peer_id)
            .map_or(0, BoundedInventory::len)
    }

    #[cfg(test)]
    pub(crate) fn recent_relay_len(&self) -> usize {
        self.recent_relay.len()
    }

    #[cfg(test)]
    pub(crate) fn was_recently_relayed(&self, item: InventoryItem) -> bool {
        self.recent_relay.contains(item)
    }
}

pub(crate) fn object_request_commands(peer_id: PeerId, item: InventoryItem) -> Vec<PeerCommand> {
    vec![
        PeerCommand::Expect {
            peer_id,
            key: RequestKey::Object(item),
        },
        PeerCommand::Send {
            peer_id,
            message: Message::GetData(vec![item]),
            class: QueueClass::RequiredData,
        },
    ]
}

pub(crate) fn validated_relay<T, E>(
    kind: InventoryKind,
    hash: Hash256,
    result: &Result<T, E>,
) -> Option<ValidatedRelay> {
    result.as_ref().ok().map(|_| ValidatedRelay {
        item: InventoryItem { kind, hash },
    })
}
