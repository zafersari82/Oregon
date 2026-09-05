pub mod support;

use std::time::Duration;

use oregon_mempool::MempoolConfig;
use oregon_network::TcpTransport;
use oregon_node::{NodeNetwork, OregonNode};
use oregon_peer::{
    PeerEvent, PeerId, PerformanceSnapshot, QueueClass, RESPONSE_START_TIMEOUT, RequestKey,
    RequestRegistry,
};
use oregon_primitives::Hash256;
use oregon_protocol::{InventoryItem, InventoryKind, Message};
use oregon_sync::{BlockScheduler, ChainSyncView, SyncAction, SyncPeer};
use tokio::time::Instant;

use support::{
    AcceptAllSpends, TestDir, accepted_state, chain_config, hello, linear_chain,
    linear_chain_with_nonce_offset, magic, node_config,
};

fn sync_peer(peer_id: PeerId, performance: PerformanceSnapshot) -> SyncPeer {
    SyncPeer {
        peer_id,
        block_relay: true,
        sync_eligible: true,
        performance,
    }
}

fn requested_peer(actions: &[SyncAction], expected_block: Hash256) -> PeerId {
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        SyncAction::RequestBlock { peer_id, block_id } => {
            assert_eq!(*block_id, expected_block);
            *peer_id
        }
        other => panic!("expected RequestBlock, got {other:?}"),
    }
}

#[tokio::test]
async fn remote_advertised_height_cannot_override_chainstate_preferred_fork_choice() {
    let dir = TestDir::scoped("resilience", "lying-height");
    let config = chain_config();
    let chain_id = config.anchor_header.block_id();
    let local_blocks = linear_chain(&config, 3);
    let fork_blocks = linear_chain_with_nonce_offset(&config, 2, 10_000);
    let local_tip = local_blocks.last().unwrap().header.block_id();
    let fork_tip = fork_blocks.last().unwrap().header.block_id();
    assert_ne!(local_tip, fork_tip);

    let state = accepted_state(&dir, &config, &local_blocks);
    let node = OregonNode::new(
        state,
        MempoolConfig::default(),
        AcceptAllSpends,
        TcpTransport,
    )
    .await
    .unwrap();

    let mut local_hello = hello(chain_id, 0x71);
    local_hello.best_height = 3;
    local_hello.best_block_id = local_tip;
    let local_network = NodeNetwork::bind(
        TcpTransport,
        node_config(),
        local_hello,
        magic(chain_id),
    )
    .await
    .unwrap();

    let mut remote_hello = hello(chain_id, 0x72);
    remote_hello.best_height = 50_000;
    remote_hello.best_block_id = fork_tip;
    let remote_network = NodeNetwork::bind(
        TcpTransport,
        node_config(),
        remote_hello,
        magic(chain_id),
    )
    .await
    .unwrap();

    let local_addr = local_network.local_addr();
    let (remote_outcome, local_outcome) =
        tokio::join!(remote_network.connect(local_addr), local_network.accept());
    let mut remote_session = remote_outcome.unwrap().session;
    let mut local_session = local_outcome.unwrap().session;

    assert_eq!(local_session.peer().remote_best_height, 50_000);
    assert_eq!(local_session.peer().remote_best_block_id, fork_tip);

    let before = node.sync_view().preferred_header_tip().await.unwrap();
    assert_eq!(before.height, 3);
    assert_eq!(before.block_id, local_tip);

    local_session.expect(RequestKey::Headers).unwrap();
    let fork_headers: Vec<_> = fork_blocks.iter().map(|block| block.header.clone()).collect();
    assert!(
        remote_session
            .send(&Message::Headers(fork_headers), QueueClass::RequiredData)
            .await
            .unwrap()
    );

    let received = match local_session.read_event().await.unwrap() {
        PeerEvent::MatchedResponse {
            key: RequestKey::Headers,
            message: Message::Headers(headers),
            ..
        } => headers,
        other => panic!("expected matched competing headers, got {other:?}"),
    };
    let outcomes = node.submit_headers(received).await.unwrap();
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes.iter().all(Result::is_ok));

    let after = node.sync_view().preferred_header_tip().await.unwrap();
    let active = node.sync_view().active_tip().await.unwrap();
    assert_eq!(after, before);
    assert_eq!(active, before);
    assert_ne!(after.block_id, fork_tip);
}

#[test]
fn nonresponding_peer_reassigns_at_twenty_seconds_and_third_timeout_stalls() {
    let target = Hash256::from_bytes([0x77; 32]);
    let key = RequestKey::Object(InventoryItem {
        kind: InventoryKind::Block,
        hash: target,
    });
    let peer_one = PeerId(1);
    let peer_two = PeerId(2);
    let mut scheduler = BlockScheduler::new(vec![target]).unwrap();
    let mut requests_one = RequestRegistry::default();
    let mut requests_two = RequestRegistry::default();
    let base = Instant::now();

    let first = scheduler.schedule(&[
        sync_peer(peer_one, requests_one.performance()),
        sync_peer(peer_two, requests_two.performance()),
    ]);
    assert_eq!(requested_peer(&first, target), peer_one);
    requests_one.expect_at(key, base).unwrap();
    assert!(
        requests_one
            .expire_at(base + RESPONSE_START_TIMEOUT - Duration::from_nanos(1))
            .is_empty()
    );
    assert_eq!(
        requests_one.expire_at(base + RESPONSE_START_TIMEOUT),
        vec![key]
    );
    assert!(scheduler.on_timeout(peer_one, target).is_empty());
    assert_eq!(scheduler.in_flight_len(), 0);

    let second = scheduler.schedule(&[
        sync_peer(peer_one, requests_one.performance()),
        sync_peer(peer_two, requests_two.performance()),
    ]);
    assert_eq!(requested_peer(&second, target), peer_two);
    let second_started = base + Duration::from_secs(30);
    requests_two.expect_at(key, second_started).unwrap();
    assert_eq!(
        requests_two.expire_at(second_started + RESPONSE_START_TIMEOUT),
        vec![key]
    );
    assert!(scheduler.on_timeout(peer_two, target).is_empty());

    let third = scheduler.schedule(&[
        sync_peer(peer_one, requests_one.performance()),
        sync_peer(peer_two, requests_two.performance()),
    ]);
    assert_eq!(requested_peer(&third, target), peer_one);
    let third_started = base + Duration::from_secs(60);
    requests_one.expect_at(key, third_started).unwrap();
    assert_eq!(
        requests_one.expire_at(third_started + RESPONSE_START_TIMEOUT),
        vec![key]
    );
    assert_eq!(
        scheduler.on_timeout(peer_one, target),
        vec![SyncAction::Stalled { block_id: target }]
    );
    assert!(
        scheduler
            .schedule(&[
                sync_peer(peer_one, requests_one.performance()),
                sync_peer(peer_two, requests_two.performance()),
            ])
            .is_empty()
    );
}
