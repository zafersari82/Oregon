mod support;

use oregon_chainstate::{AcceptOutcome, ChainState};
use oregon_mempool::MempoolConfig;
use oregon_network::TcpTransport;
use oregon_node::{NodeNetwork, OregonNode};
use oregon_peer::{PeerEvent, QueueClass, RequestKey};
use oregon_primitives::Hash256;
use oregon_protocol::{FeatureSet, InventoryItem, InventoryKind, Message};
use oregon_sync::{
    BlockScheduler, ChainSyncView, MAX_IN_FLIGHT_BLOCKS_GLOBAL, MAX_IN_FLIGHT_BLOCKS_PEER,
    SyncAction, SyncPeer, build_locator, headers_after_common_height, highest_locator_hit,
    missing_body_targets, validate_headers_response,
};

use support::{
    AcceptAllSpends, TestDir, accepted_state, chain_config, hello, linear_chain, magic,
    node_config,
};

async fn preferred_path<V: ChainSyncView + ?Sized>(view: &V) -> Vec<(u64, Hash256)> {
    let tip = view.preferred_header_tip().await.unwrap();
    let mut path = Vec::with_capacity((tip.height + 1) as usize);
    for height in 0..=tip.height {
        let block_id = view
            .preferred_header_id_at_height(height)
            .await
            .unwrap()
            .expect("preferred path height must exist");
        path.push((height, block_id));
    }
    path
}

#[tokio::test]
async fn behind_node_catches_up_headers_first_through_validating_middle_peer() {
    let a_dir = TestDir::scoped("sync", "a");
    let b_dir = TestDir::scoped("sync", "b");
    let c_dir = TestDir::scoped("sync", "c");
    let config = chain_config();
    let blocks = linear_chain(&config, 4);
    let expected_tip = blocks.last().unwrap().header.block_id();
    let chain_id = config.anchor_header.block_id();

    let a_state = accepted_state(&a_dir, &config, &blocks);
    let b_state = ChainState::open(b_dir.path(), config.clone()).unwrap();
    let c_state = ChainState::open(c_dir.path(), config.clone()).unwrap();
    let a_node = OregonNode::new(
        a_state,
        MempoolConfig::default(),
        AcceptAllSpends,
        TcpTransport,
    )
    .await
    .unwrap();
    let b_node = OregonNode::new(
        b_state,
        MempoolConfig::default(),
        AcceptAllSpends,
        TcpTransport,
    )
    .await
    .unwrap();
    let c_node = OregonNode::new(
        c_state,
        MempoolConfig::default(),
        AcceptAllSpends,
        TcpTransport,
    )
    .await
    .unwrap();

    let mut a_hello = hello(chain_id, 0x10);
    a_hello.best_height = 4;
    a_hello.best_block_id = expected_tip;
    let b_network = NodeNetwork::bind(
        TcpTransport,
        node_config(),
        hello(chain_id, 0x20),
        magic(chain_id),
    )
    .await
    .unwrap();
    let a_network = NodeNetwork::bind(TcpTransport, node_config(), a_hello, magic(chain_id))
        .await
        .unwrap();
    let c_network = NodeNetwork::bind(
        TcpTransport,
        node_config(),
        hello(chain_id, 0x30),
        magic(chain_id),
    )
    .await
    .unwrap();

    let b_addr = b_network.local_addr();
    let (a_outcome, b_from_a_outcome) =
        tokio::join!(a_network.connect(b_addr), b_network.accept());
    let (c_outcome, b_from_c_outcome) =
        tokio::join!(c_network.connect(b_addr), b_network.accept());
    let mut a_to_b = a_outcome.unwrap().session;
    let mut b_from_a = b_from_a_outcome.unwrap().session;
    let mut c_to_b = c_outcome.unwrap().session;
    let mut b_from_c = b_from_c_outcome.unwrap().session;

    assert!(c_to_b.peer().features.contains(FeatureSet::HEADERS_SYNC));
    assert!(c_to_b.peer().features.contains(FeatureSet::BLOCK_RELAY));

    let c_request = build_locator(&c_node.sync_view(), None).await.unwrap();
    c_to_b.expect(RequestKey::Headers).unwrap();
    assert!(
        c_to_b
            .send(
                &Message::GetHeaders(c_request.clone()),
                QueueClass::RequiredData,
            )
            .await
            .unwrap()
    );
    let forwarded_request = match b_from_c.read_event().await.unwrap() {
        PeerEvent::Message {
            message: Message::GetHeaders(request),
            ..
        } => request,
        other => panic!("expected GetHeaders at B, got {other:?}"),
    };
    assert_eq!(forwarded_request, c_request);

    b_from_a.expect(RequestKey::Headers).unwrap();
    assert!(
        b_from_a
            .send(
                &Message::GetHeaders(forwarded_request.clone()),
                QueueClass::RequiredData,
            )
            .await
            .unwrap()
    );
    let request_at_a = match a_to_b.read_event().await.unwrap() {
        PeerEvent::Message {
            message: Message::GetHeaders(request),
            ..
        } => request,
        other => panic!("expected forwarded GetHeaders at A, got {other:?}"),
    };

    let a_view = a_node.sync_view();
    let a_path = preferred_path(&a_view).await;
    let (common_height, common_id) = highest_locator_hit(&request_at_a.locator, &a_path)
        .expect("C locator must intersect A preferred path");
    let headers = headers_after_common_height(&a_view, common_height, request_at_a.stop)
        .await
        .unwrap();
    assert_eq!(headers.len(), blocks.len());
    validate_headers_response(common_id, &headers).unwrap();
    assert!(
        a_to_b
            .send(&Message::Headers(headers.clone()), QueueClass::RequiredData)
            .await
            .unwrap()
    );

    let headers_at_b = match b_from_a.read_event().await.unwrap() {
        PeerEvent::MatchedResponse {
            key: RequestKey::Headers,
            message: Message::Headers(headers),
            ..
        } => headers,
        other => panic!("expected matched Headers at B, got {other:?}"),
    };
    let b_path = preferred_path(&b_node.sync_view()).await;
    let (_, b_common_id) = highest_locator_hit(&forwarded_request.locator, &b_path)
        .expect("C locator must intersect B preferred path");
    validate_headers_response(b_common_id, &headers_at_b).unwrap();
    let b_header_results = b_node.submit_headers(headers_at_b.clone()).await.unwrap();
    assert_eq!(b_header_results.len(), blocks.len());
    assert!(b_header_results.iter().all(Result::is_ok));
    let b_after_headers = b_node.sync_view();
    assert_eq!(b_after_headers.active_tip().await.unwrap().height, 0);
    assert_eq!(
        b_after_headers.preferred_header_tip().await.unwrap().block_id,
        expected_tip
    );

    assert!(
        b_from_c
            .send(
                &Message::Headers(headers_at_b.clone()),
                QueueClass::RequiredData,
            )
            .await
            .unwrap()
    );
    let headers_at_c = match c_to_b.read_event().await.unwrap() {
        PeerEvent::MatchedResponse {
            key: RequestKey::Headers,
            message: Message::Headers(headers),
            ..
        } => headers,
        other => panic!("expected matched Headers at C, got {other:?}"),
    };
    let c_path = preferred_path(&c_node.sync_view()).await;
    let (_, c_common_id) = highest_locator_hit(&c_request.locator, &c_path)
        .expect("C locator must intersect its own preferred path");
    validate_headers_response(c_common_id, &headers_at_c).unwrap();
    let c_header_results = c_node.submit_headers(headers_at_c).await.unwrap();
    assert_eq!(c_header_results.len(), blocks.len());
    assert!(c_header_results.iter().all(Result::is_ok));
    let c_after_headers = c_node.sync_view();
    assert_eq!(c_after_headers.active_tip().await.unwrap().height, 0);
    assert_eq!(
        c_after_headers.preferred_header_tip().await.unwrap().block_id,
        expected_tip
    );

    let targets = missing_body_targets(&c_after_headers).await.unwrap();
    let expected_targets: Vec<_> = blocks.iter().map(|block| block.header.block_id()).collect();
    assert_eq!(targets, expected_targets);
    let mut scheduler = BlockScheduler::new(targets).unwrap();
    let c_peer_id = c_to_b.peer().peer_id;
    let sync_peer = SyncPeer {
        peer_id: c_peer_id,
        block_relay: c_to_b.peer().features.contains(FeatureSet::BLOCK_RELAY),
        sync_eligible: c_to_b.sync_eligible(),
        performance: c_to_b.performance(),
    };
    let requests = scheduler.schedule(&[sync_peer]);
    assert_eq!(requests.len(), blocks.len());
    assert!(scheduler.in_flight_len() <= MAX_IN_FLIGHT_BLOCKS_GLOBAL);
    assert!(scheduler.in_flight_for_peer(c_peer_id) <= MAX_IN_FLIGHT_BLOCKS_PEER);

    for action in requests {
        let SyncAction::RequestBlock { peer_id, block_id } = action else {
            panic!("scheduler must emit only RequestBlock during initial fill");
        };
        assert_eq!(peer_id, c_peer_id);
        let item = InventoryItem {
            kind: InventoryKind::Block,
            hash: block_id,
        };
        let block = blocks
            .iter()
            .find(|block| block.header.block_id() == block_id)
            .expect("scheduled block must exist in fixture")
            .clone();

        c_to_b.expect(RequestKey::Object(item)).unwrap();
        assert!(
            c_to_b
                .send(&Message::GetData(vec![item]), QueueClass::RequiredData)
                .await
                .unwrap()
        );
        match b_from_c.read_event().await.unwrap() {
            PeerEvent::Message {
                message: Message::GetData(items),
                ..
            } => assert_eq!(items, vec![item]),
            other => panic!("expected GetData at B, got {other:?}"),
        }

        b_from_a.expect(RequestKey::Object(item)).unwrap();
        assert!(
            b_from_a
                .send(&Message::GetData(vec![item]), QueueClass::RequiredData)
                .await
                .unwrap()
        );
        match a_to_b.read_event().await.unwrap() {
            PeerEvent::Message {
                message: Message::GetData(items),
                ..
            } => assert_eq!(items, vec![item]),
            other => panic!("expected forwarded GetData at A, got {other:?}"),
        }
        assert!(
            a_to_b
                .send(&Message::Block(block), QueueClass::RequiredData)
                .await
                .unwrap()
        );

        let block_at_b = match b_from_a.read_event().await.unwrap() {
            PeerEvent::MatchedResponse {
                key: RequestKey::Object(matched_item),
                message: Message::Block(block),
                ..
            } => {
                assert_eq!(matched_item, item);
                block
            }
            other => panic!("expected matched Block at B, got {other:?}"),
        };
        let b_submission = b_node.submit_block(block_at_b.clone()).await.unwrap();
        assert!(matches!(b_submission.result, Ok(AcceptOutcome::Extended)));
        assert!(b_submission.relay_authorization.is_some());
        assert!(
            b_from_c
                .send(&Message::Block(block_at_b), QueueClass::RequiredData)
                .await
                .unwrap()
        );

        let block_at_c = match c_to_b.read_event().await.unwrap() {
            PeerEvent::MatchedResponse {
                key: RequestKey::Object(matched_item),
                message: Message::Block(block),
                ..
            } => {
                assert_eq!(matched_item, item);
                block
            }
            other => panic!("expected matched Block at C, got {other:?}"),
        };
        let submissions = scheduler.on_block(c_peer_id, block_at_c).unwrap();
        assert_eq!(submissions.len(), 1);
        let SyncAction::SubmitBlock { source, block } = submissions.into_iter().next().unwrap()
        else {
            panic!("in-order body must emit SubmitBlock");
        };
        assert_eq!(source, c_peer_id);
        let c_submission = c_node.submit_block(block).await.unwrap();
        assert!(matches!(c_submission.result, Ok(AcceptOutcome::Extended)));
        assert!(scheduler.in_flight_len() <= MAX_IN_FLIGHT_BLOCKS_GLOBAL);
        assert!(scheduler.in_flight_for_peer(c_peer_id) <= MAX_IN_FLIGHT_BLOCKS_PEER);
    }

    assert!(scheduler.is_complete());
    let a_tip = a_node.sync_view().active_tip().await.unwrap();
    let b_tip = b_node.sync_view().active_tip().await.unwrap();
    let c_tip = c_node.sync_view().active_tip().await.unwrap();
    assert_eq!(a_tip.block_id, expected_tip);
    assert_eq!(a_tip.height, 4);
    assert_eq!(b_tip, a_tip);
    assert_eq!(c_tip, a_tip);
    assert_eq!(
        b_node.sync_view().preferred_header_tip().await.unwrap(),
        a_node.sync_view().preferred_header_tip().await.unwrap()
    );
    assert_eq!(
        c_node.sync_view().preferred_header_tip().await.unwrap(),
        a_node.sync_view().preferred_header_tip().await.unwrap()
    );
}
