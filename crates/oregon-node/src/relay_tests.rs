use oregon_chainstate::{AcceptOutcome, ChainStateError};
use oregon_mempool::{AdmissionOutcome, MempoolError};
use oregon_peer::{PeerCommand, PeerId, QueueClass, RequestKey};
use oregon_primitives::Hash256;
use oregon_protocol::{InventoryItem, InventoryKind, Message};

use crate::relay::{
    MAX_KNOWN_INVENTORY_PER_PEER, MAX_RECENT_RELAY_CACHE, RelayState, object_request_commands,
    validated_inventory,
};

fn inventory(sequence: u64) -> InventoryItem {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&sequence.to_le_bytes());
    InventoryItem {
        kind: InventoryKind::Transaction,
        hash: Hash256::from_bytes(bytes),
    }
}

#[test]
fn rejected_transaction_never_authorizes_inventory_relay() {
    let txid = Hash256::from_bytes([0x11; 32]);
    let result: Result<AdmissionOutcome, MempoolError> = Err(MempoolError::AlreadyKnown(txid));
    assert_eq!(
        validated_inventory(InventoryKind::Transaction, txid, &result),
        None
    );
}

#[test]
fn accepted_transaction_authorizes_inventory_relay() {
    let txid = Hash256::from_bytes([0x22; 32]);
    let result: Result<AdmissionOutcome, MempoolError> = Ok(AdmissionOutcome {
        txid,
        fee: 1,
        encoded_bytes: 42,
        evicted: Vec::new(),
    });
    assert_eq!(
        validated_inventory(InventoryKind::Transaction, txid, &result),
        Some(InventoryItem {
            kind: InventoryKind::Transaction,
            hash: txid,
        })
    );
}

#[test]
fn invalid_block_never_authorizes_inventory_relay() {
    let block_id = Hash256::from_bytes([0x33; 32]);
    let result: Result<AcceptOutcome, ChainStateError> = Err(ChainStateError::UnknownParent(
        Hash256::from_bytes([0x44; 32]),
    ));
    assert_eq!(
        validated_inventory(InventoryKind::Block, block_id, &result),
        None
    );
}

#[test]
fn accepted_sidechain_block_authorizes_inventory_relay() {
    let block_id = Hash256::from_bytes([0x55; 32]);
    let result: Result<AcceptOutcome, ChainStateError> = Ok(AcceptOutcome::StoredSideChain);
    assert_eq!(
        validated_inventory(InventoryKind::Block, block_id, &result),
        Some(InventoryItem {
            kind: InventoryKind::Block,
            hash: block_id,
        })
    );
}

#[test]
fn relay_bounds_are_exact() {
    assert_eq!(MAX_KNOWN_INVENTORY_PER_PEER, 8_192);
    assert_eq!(MAX_RECENT_RELAY_CACHE, 65_536);
}

#[test]
fn per_peer_known_inventory_evicts_oldest_generation_at_exact_cap() {
    let peer = PeerId(7);
    let mut relay = RelayState::default();

    for sequence in 0..MAX_KNOWN_INVENTORY_PER_PEER as u64 {
        assert!(relay.note_peer_inventory(peer, inventory(sequence)));
    }
    assert_eq!(
        relay.known_inventory_len(peer),
        MAX_KNOWN_INVENTORY_PER_PEER
    );
    assert!(relay.peer_knows(peer, inventory(0)));

    assert!(relay.note_peer_inventory(peer, inventory(MAX_KNOWN_INVENTORY_PER_PEER as u64)));
    assert_eq!(
        relay.known_inventory_len(peer),
        MAX_KNOWN_INVENTORY_PER_PEER
    );
    assert!(!relay.peer_knows(peer, inventory(0)));
    assert!(relay.peer_knows(peer, inventory(MAX_KNOWN_INVENTORY_PER_PEER as u64)));

    assert!(!relay.note_peer_inventory(peer, inventory(MAX_KNOWN_INVENTORY_PER_PEER as u64)));
    assert_eq!(
        relay.known_inventory_len(peer),
        MAX_KNOWN_INVENTORY_PER_PEER
    );
}

#[test]
fn recent_relay_cache_evicts_oldest_generation_at_exact_cap() {
    let mut relay = RelayState::default();

    for sequence in 0..MAX_RECENT_RELAY_CACHE as u64 {
        assert!(relay.note_recent_relay(inventory(sequence)));
    }
    assert_eq!(relay.recent_relay_len(), MAX_RECENT_RELAY_CACHE);
    assert!(relay.was_recently_relayed(inventory(0)));

    assert!(relay.note_recent_relay(inventory(MAX_RECENT_RELAY_CACHE as u64)));
    assert_eq!(relay.recent_relay_len(), MAX_RECENT_RELAY_CACHE);
    assert!(!relay.was_recently_relayed(inventory(0)));
    assert!(relay.was_recently_relayed(inventory(MAX_RECENT_RELAY_CACHE as u64)));
}

#[test]
fn authorized_relay_excludes_source_and_known_peers_and_marks_recipients_known() {
    let source = PeerId(1);
    let known = PeerId(2);
    let recipient = PeerId(3);
    let item = inventory(42);
    let mut relay = RelayState::default();
    relay.note_peer_inventory(known, item);

    let commands = relay.relay_inventory(Some(source), [source, known, recipient], item);

    assert_eq!(
        commands,
        vec![PeerCommand::Send {
            peer_id: recipient,
            message: Message::Inv(vec![item]),
            class: QueueClass::Gossip,
        }]
    );
    assert!(relay.peer_knows(source, item));
    assert!(relay.peer_knows(known, item));
    assert!(relay.peer_knows(recipient, item));
    assert!(relay.was_recently_relayed(item));
    assert!(
        relay
            .relay_inventory(Some(source), [source, known, recipient], item)
            .is_empty()
    );
}

#[test]
fn object_request_registers_expect_before_required_getdata_send() {
    let peer = PeerId(9);
    let item = inventory(99);

    assert_eq!(
        object_request_commands(peer, item),
        vec![
            PeerCommand::Expect {
                peer_id: peer,
                key: RequestKey::Object(item),
            },
            PeerCommand::Send {
                peer_id: peer,
                message: Message::GetData(vec![item]),
                class: QueueClass::RequiredData,
            },
        ]
    );
}
