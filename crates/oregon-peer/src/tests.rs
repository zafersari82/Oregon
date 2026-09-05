use std::time::Duration;

use oregon_protocol::{FeatureSet, Hash256, Hello, HelloAck, Message};

use crate::budget::{GlobalQueueBudget, PeerQueueBudget};
use crate::handshake::HandshakeMachine;
use crate::service::PendingHandshakes;
use crate::{
    CONTROL_RESERVED_BYTES, CONTROL_RESERVED_FRAMES, Direction, HANDSHAKE_TIMEOUT,
    MAX_PENDING_HANDSHAKES, MAX_QUEUE_BYTES_GLOBAL, MAX_QUEUE_BYTES_PEER, MAX_QUEUE_FRAMES_PEER,
    PeerConfig, PeerError, QUEUE_ENQUEUE_TIMEOUT, QueueClass, generate_process_nonce,
    preferred_direction,
};

fn hello(nonce: [u8; 16], chain_byte: u8) -> Hello {
    Hello {
        min_protocol_version: 1,
        max_protocol_version: 1,
        chain_id: Hash256::from_bytes([chain_byte; 32]),
        instance_nonce: nonce,
        offered_features: FeatureSet::HEADERS_SYNC
            | FeatureSet::BLOCK_RELAY
            | FeatureSet::TX_RELAY,
        required_features: FeatureSet::HEADERS_SYNC,
        best_height: 7,
        best_block_id: Hash256::from_bytes([9; 32]),
    }
}

#[test]
fn peer_config_rejects_invalid_sums_and_hard_limit() {
    assert_eq!(
        PeerConfig::new(64, 16, 49),
        Err(PeerError::InvalidConfig(
            "inbound + outbound exceeds max_peers"
        ))
    );
    assert_eq!(
        PeerConfig::new(129, 16, 48),
        Err(PeerError::InvalidConfig("max_peers exceeds hard limit"))
    );
    assert_eq!(PeerConfig::new(64, 16, 48).unwrap().max_peers, 64);
}

#[test]
fn queue_frame_reservation_is_inside_exact_peer_cap() {
    let global = GlobalQueueBudget::new();
    let peer = PeerQueueBudget::new(global);
    let mut data = Vec::new();
    for _ in 0..(MAX_QUEUE_FRAMES_PEER - CONTROL_RESERVED_FRAMES) {
        data.push(
            peer.try_reserve(QueueClass::RequiredData, 1)
                .unwrap()
                .expect("data frame within non-control cap"),
        );
    }
    assert!(
        peer.try_reserve(QueueClass::RequiredData, 1)
            .unwrap()
            .is_none()
    );

    let mut control = Vec::new();
    for _ in 0..CONTROL_RESERVED_FRAMES {
        control.push(
            peer.try_reserve(QueueClass::Control, 1)
                .unwrap()
                .expect("control reservation remains available"),
        );
    }
    assert!(peer.try_reserve(QueueClass::Control, 1).unwrap().is_none());
    assert_eq!(peer.snapshot().0, MAX_QUEUE_FRAMES_PEER);
}

#[test]
fn queue_byte_reservation_preserves_exact_control_bytes() {
    let global = GlobalQueueBudget::new();
    let peer = PeerQueueBudget::new(global);
    let _data = peer
        .try_reserve(
            QueueClass::RequiredData,
            MAX_QUEUE_BYTES_PEER - CONTROL_RESERVED_BYTES,
        )
        .unwrap()
        .expect("non-control bytes fit exactly");
    assert!(
        peer.try_reserve(QueueClass::RequiredData, 1)
            .unwrap()
            .is_none()
    );
    let _control = peer
        .try_reserve(QueueClass::Control, CONTROL_RESERVED_BYTES)
        .unwrap()
        .expect("reserved control bytes fit exactly");
    assert!(peer.try_reserve(QueueClass::Control, 1).unwrap().is_none());
    assert_eq!(peer.snapshot().1, MAX_QUEUE_BYTES_PEER);
}

#[test]
fn global_queue_byte_cap_is_exact() {
    let global = GlobalQueueBudget::new();
    let mut permits = Vec::new();
    let chunk = 2 * 1024 * 1024;
    for _ in 0..(MAX_QUEUE_BYTES_GLOBAL / chunk) {
        let peer = PeerQueueBudget::new(global.clone());
        permits.push(
            peer.try_reserve(QueueClass::Control, chunk)
                .unwrap()
                .expect("global capacity remains"),
        );
    }
    let extra = PeerQueueBudget::new(global);
    assert!(
        extra
            .try_reserve(QueueClass::Control, 1)
            .unwrap()
            .is_none()
    );
}

#[test]
fn handshake_machine_requires_hello_then_matching_ack() {
    let local = hello([1; 16], 7);
    let remote = hello([2; 16], 7);
    let mut machine = HandshakeMachine::new(local.clone());
    assert_eq!(machine.state(), crate::HandshakeState::Connected);
    assert!(matches!(machine.start().unwrap(), Message::Hello(_)));
    let response = machine
        .on_message(Message::Hello(remote.clone()))
        .unwrap()
        .expect("Hello produces HelloAck");
    let Message::HelloAck(local_ack) = response else {
        panic!("expected HelloAck");
    };
    assert_eq!(local_ack.remote_nonce_echo, remote.instance_nonce);
    let negotiated = machine.negotiated().unwrap();
    machine
        .on_message(Message::HelloAck(HelloAck {
            selected_protocol_version: negotiated.protocol_version,
            enabled_features: negotiated.features,
            remote_nonce_echo: local.instance_nonce,
        }))
        .unwrap();
    assert_eq!(machine.state(), crate::HandshakeState::Established);
}

#[test]
fn pre_established_gossip_is_a_handshake_violation() {
    let mut machine = HandshakeMachine::new(hello([1; 16], 7));
    machine.start().unwrap();
    assert_eq!(
        machine.on_message(Message::Inv(Vec::new())).unwrap_err(),
        PeerError::HandshakeViolation("application message received before Established")
    );
}

#[test]
fn handshake_rejects_self_wrong_chain_and_ack_mismatch() {
    let local = hello([1; 16], 7);

    let mut self_peer = HandshakeMachine::new(local.clone());
    self_peer.start().unwrap();
    assert_eq!(
        self_peer
            .on_message(Message::Hello(hello([1; 16], 7)))
            .unwrap_err(),
        PeerError::SelfPeer
    );

    let mut wrong_chain = HandshakeMachine::new(local.clone());
    wrong_chain.start().unwrap();
    assert_eq!(
        wrong_chain
            .on_message(Message::Hello(hello([2; 16], 8)))
            .unwrap_err(),
        PeerError::WrongChain
    );

    let mut mismatch = HandshakeMachine::new(local.clone());
    mismatch.start().unwrap();
    mismatch
        .on_message(Message::Hello(hello([2; 16], 7)))
        .unwrap();
    let negotiated = mismatch.negotiated().unwrap();
    assert_eq!(
        mismatch
            .on_message(Message::HelloAck(HelloAck {
                selected_protocol_version: negotiated.protocol_version,
                enabled_features: negotiated.features,
                remote_nonce_echo: [3; 16],
            }))
            .unwrap_err(),
        PeerError::AckMismatch
    );
}

#[test]
fn duplicate_arbitration_is_symmetric() {
    let small = [1; 16];
    let large = [2; 16];
    assert_eq!(
        preferred_direction(small, large).unwrap(),
        Direction::Outbound
    );
    assert_eq!(
        preferred_direction(large, small).unwrap(),
        Direction::Inbound
    );
    assert_eq!(preferred_direction(small, small), Err(PeerError::SelfPeer));
}

#[test]
fn pending_handshake_cap_is_exact() {
    let pending = PendingHandshakes::standalone();
    let mut guards = Vec::new();
    for _ in 0..MAX_PENDING_HANDSHAKES {
        guards.push(pending.acquire().unwrap());
    }
    assert_eq!(pending.acquire().unwrap_err(), PeerError::PendingHandshakeLimit);
    guards.pop();
    assert!(pending.acquire().is_ok());
}

#[test]
fn frozen_peer_time_bounds_are_exact() {
    assert_eq!(HANDSHAKE_TIMEOUT, Duration::from_secs(10));
    assert_eq!(QUEUE_ENQUEUE_TIMEOUT, Duration::from_secs(2));
}

#[test]
fn process_nonce_comes_from_os_entropy_api() {
    assert_eq!(generate_process_nonce().unwrap().len(), 16);
}
