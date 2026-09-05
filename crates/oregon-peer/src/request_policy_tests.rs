use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use oregon_protocol::{Hash256, InventoryItem, InventoryKind};
use tokio::time::Instant;

use crate::cooldown::{CooldownTable, DISCONNECT_COOLDOWN, MAX_COOLDOWN_ENTRIES};
use crate::request::{
    EXPIRED_REQUEST_GRACE, MAX_RECENTLY_EXPIRED_REQUESTS_PER_PEER, RESPONSE_START_TIMEOUT,
    RequestError, RequestKey, RequestRegistry, ResponseDisposition,
};
use crate::score::{Misbehavior, PeerScore, ScoreDecision};
use crate::service::{IDLE_TIMEOUT, LivenessAction, LivenessState, PING_INTERVAL, PONG_TIMEOUT};

fn object(index: u16) -> RequestKey {
    let mut bytes = [0u8; 32];
    bytes[..2].copy_from_slice(&index.to_le_bytes());
    RequestKey::Object(InventoryItem {
        kind: InventoryKind::Block,
        hash: Hash256::from_bytes(bytes),
    })
}

#[test]
fn frozen_request_and_liveness_bounds_are_exact() {
    assert_eq!(RESPONSE_START_TIMEOUT, Duration::from_secs(20));
    assert_eq!(EXPIRED_REQUEST_GRACE, Duration::from_secs(30));
    assert_eq!(MAX_RECENTLY_EXPIRED_REQUESTS_PER_PEER, 128);
    assert_eq!(PING_INTERVAL, Duration::from_secs(30));
    assert_eq!(PONG_TIMEOUT, Duration::from_secs(15));
    assert_eq!(IDLE_TIMEOUT, Duration::from_secs(120));
    assert_eq!(DISCONNECT_COOLDOWN, Duration::from_secs(600));
    assert_eq!(MAX_COOLDOWN_ENTRIES, 1_024);
}

#[test]
fn only_one_headers_request_may_be_outstanding() {
    let now = Instant::now();
    let mut requests = RequestRegistry::default();
    requests.expect_at(RequestKey::Headers, now).unwrap();
    assert_eq!(
        requests.expect_at(RequestKey::Headers, now),
        Err(RequestError::AlreadyOutstanding(RequestKey::Headers))
    );
}

#[test]
fn object_response_matches_by_kind_and_hash() {
    let now = Instant::now();
    let expected = object(7);
    let wrong_hash = object(8);
    let RequestKey::Object(item) = expected else {
        unreachable!();
    };
    let wrong_kind = RequestKey::Object(InventoryItem {
        kind: InventoryKind::Transaction,
        hash: item.hash,
    });

    let mut requests = RequestRegistry::default();
    requests.expect_at(expected, now).unwrap();
    assert_eq!(
        requests.classify_key_at(wrong_hash, now + Duration::from_secs(1)),
        ResponseDisposition::Unsolicited(wrong_hash)
    );
    assert_eq!(
        requests.classify_key_at(wrong_kind, now + Duration::from_secs(1)),
        ResponseDisposition::Unsolicited(wrong_kind)
    );
    assert_eq!(
        requests.classify_key_at(expected, now + Duration::from_millis(1_500)),
        ResponseDisposition::Matched(expected)
    );
    let performance = requests.performance();
    assert_eq!(performance.success_count, 1);
    assert_eq!(performance.timeout_count, 0);
    assert_eq!(performance.average_response_latency_ms, 1_500);
}

#[test]
fn timeout_moves_request_to_grace_and_late_response_is_not_an_offense() {
    let now = Instant::now();
    let key = object(9);
    let mut requests = RequestRegistry::default();
    requests.expect_at(key, now).unwrap();

    assert!(
        requests
            .expire_at(now + RESPONSE_START_TIMEOUT - Duration::from_millis(1))
            .is_empty()
    );
    assert_eq!(requests.expire_at(now + RESPONSE_START_TIMEOUT), vec![key]);
    assert_eq!(requests.performance().timeout_count, 1);
    assert_eq!(
        requests.classify_key_at(
            key,
            now + RESPONSE_START_TIMEOUT + EXPIRED_REQUEST_GRACE - Duration::from_millis(1)
        ),
        ResponseDisposition::GraceDrop(key)
    );
}

#[test]
fn grace_cap_evicts_the_earliest_generation_deterministically() {
    let now = Instant::now();
    let mut requests = RequestRegistry::default();
    for index in 0..=MAX_RECENTLY_EXPIRED_REQUESTS_PER_PEER as u16 {
        requests.expect_at(object(index), now).unwrap();
    }
    let expired = requests.expire_at(now + RESPONSE_START_TIMEOUT);
    assert_eq!(expired.len(), MAX_RECENTLY_EXPIRED_REQUESTS_PER_PEER + 1);
    assert_eq!(requests.grace_len(), MAX_RECENTLY_EXPIRED_REQUESTS_PER_PEER);

    assert_eq!(
        requests.classify_key_at(object(0), now + RESPONSE_START_TIMEOUT),
        ResponseDisposition::Unsolicited(object(0))
    );
    assert_eq!(
        requests.classify_key_at(object(1), now + RESPONSE_START_TIMEOUT),
        ResponseDisposition::GraceDrop(object(1))
    );
}

#[test]
fn non_grace_unsolicited_response_is_identified() {
    let now = Instant::now();
    let mut requests = RequestRegistry::default();
    let key = object(44);
    assert_eq!(
        requests.classify_key_at(key, now),
        ResponseDisposition::Unsolicited(key)
    );
}

#[test]
fn misbehavior_points_and_thresholds_are_exact() {
    let mut score = PeerScore::default();
    assert_eq!(
        score.apply(Misbehavior::MalformedFrame),
        ScoreDecision::Continue
    );
    assert_eq!(score.points(), 25);
    assert_eq!(
        score.apply(Misbehavior::HandshakeViolation),
        ScoreDecision::StopSync
    );
    assert_eq!(score.points(), 50);
    assert!(!score.sync_eligible());
    assert!(!score.disconnect_required());

    let mut individual = PeerScore::default();
    assert_eq!(individual.points_for(Misbehavior::InvalidResponse), 10);
    assert_eq!(individual.points_for(Misbehavior::UnsolicitedObject), 10);
    assert_eq!(individual.points_for(Misbehavior::RequestAbuse), 10);
    assert_eq!(individual.points_for(Misbehavior::SyncTimeout), 5);
    assert_eq!(individual.points_for(Misbehavior::InvalidHeader), 50);
    assert_eq!(individual.points_for(Misbehavior::InvalidBlock), 50);

    individual.apply(Misbehavior::InvalidHeader);
    assert_eq!(
        individual.apply(Misbehavior::InvalidBlock),
        ScoreDecision::Disconnect
    );
    assert_eq!(individual.points(), 100);
    assert!(individual.disconnect_required());
}

#[test]
fn oversized_frame_disconnects_immediately_without_score_accumulation() {
    let mut score = PeerScore::default();
    assert_eq!(
        score.apply(Misbehavior::OversizedFrame),
        ScoreDecision::Disconnect
    );
    assert_eq!(score.points(), 0);
}

#[test]
fn liveness_uses_exact_ping_pong_and_idle_deadlines() {
    let now = Instant::now();
    let mut live = LivenessState::new(now);
    assert_eq!(
        live.poll_at(now + PING_INTERVAL - Duration::from_millis(1)),
        LivenessAction::None
    );
    let LivenessAction::SendPing(nonce) = live.poll_at(now + PING_INTERVAL) else {
        panic!("ping must become due at exactly 30 seconds");
    };
    assert_eq!(
        live.poll_at(now + PING_INTERVAL + PONG_TIMEOUT - Duration::from_millis(1)),
        LivenessAction::None
    );
    assert_eq!(
        live.poll_at(now + PING_INTERVAL + PONG_TIMEOUT),
        LivenessAction::Disconnect
    );

    let mut answered = LivenessState::new(now);
    let LivenessAction::SendPing(answer_nonce) = answered.poll_at(now + PING_INTERVAL) else {
        panic!("expected ping");
    };
    assert!(answered.on_pong(answer_nonce, now + PING_INTERVAL));
    assert_ne!(nonce, 0);

    let mut idle = LivenessState::new(now);
    assert_eq!(
        idle.poll_at(now + IDLE_TIMEOUT - Duration::from_millis(1)),
        LivenessAction::SendPing(1)
    );
    assert_eq!(idle.poll_at(now + IDLE_TIMEOUT), LivenessAction::Disconnect);
}

#[test]
fn cooldown_normalizes_mapped_ipv4_and_evicts_earliest_expiry() {
    let now = Instant::now();
    let v4 = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));
    let mapped = IpAddr::V6(Ipv6Addr::from(0xffff_c000_0201u128));
    let mut cooldown = CooldownTable::default();
    cooldown.insert_at(v4, now);
    assert!(cooldown.contains_at(mapped, now));

    let mut bounded = CooldownTable::default();
    for index in 0..MAX_COOLDOWN_ENTRIES {
        bounded.insert_at(
            IpAddr::V6(Ipv6Addr::from(index as u128 + 1)),
            now + Duration::from_millis(index as u64),
        );
    }
    assert_eq!(bounded.len(), MAX_COOLDOWN_ENTRIES);
    bounded.insert_at(
        IpAddr::V6(Ipv6Addr::from(0x1_0000u128)),
        now + Duration::from_secs(2),
    );
    assert_eq!(bounded.len(), MAX_COOLDOWN_ENTRIES);
    assert!(!bounded.contains_at(
        IpAddr::V6(Ipv6Addr::from(1u128)),
        now + Duration::from_secs(2)
    ));
    assert!(bounded.contains_at(
        IpAddr::V6(Ipv6Addr::from(2u128)),
        now + Duration::from_secs(2)
    ));
}
