mod support;

use std::time::Duration;

use oregon_chainstate::{AcceptOutcome, ChainState};
use oregon_mempool::MempoolConfig;
use oregon_network::TcpTransport;
use oregon_node::{NodeNetwork, OregonNode};
use oregon_peer::{PeerCommand, PeerEvent, QueueClass, RequestKey};
use oregon_protocol::{InventoryItem, InventoryKind, Message};
use tokio::time::timeout;

use support::{
    AcceptAllSpends, TestDir, chain_config, founder_block, hello, magic, node_config, spend,
    state_with_spendable_utxo, unknown_parent_block,
};

#[tokio::test]
async fn accepted_block_relays_over_tcp_only_after_chainstate_acceptance() {
    let dir = TestDir::scoped("relay", "block");
    let config = chain_config();
    let chain_id = config.anchor_header.block_id();
    let state = ChainState::open(dir.path(), config.clone()).unwrap();
    let mut node = OregonNode::new(
        state,
        MempoolConfig::default(),
        AcceptAllSpends,
        TcpTransport,
    )
    .await
    .unwrap();

    let hub = NodeNetwork::bind(
        TcpTransport,
        node_config(),
        hello(chain_id, 0x20),
        magic(chain_id),
    )
    .await
    .unwrap();
    let source = NodeNetwork::bind(
        TcpTransport,
        node_config(),
        hello(chain_id, 0x10),
        magic(chain_id),
    )
    .await
    .unwrap();
    let downstream = NodeNetwork::bind(
        TcpTransport,
        node_config(),
        hello(chain_id, 0x30),
        magic(chain_id),
    )
    .await
    .unwrap();

    let hub_addr = hub.local_addr();
    let (source_outcome, hub_source_outcome) =
        tokio::join!(source.connect(hub_addr), hub.accept());
    let (downstream_outcome, hub_downstream_outcome) =
        tokio::join!(downstream.connect(hub_addr), hub.accept());
    let mut source_session = source_outcome.unwrap().session;
    let mut hub_source_session = hub_source_outcome.unwrap().session;
    let mut downstream_session = downstream_outcome.unwrap().session;
    let mut hub_downstream_session = hub_downstream_outcome.unwrap().session;

    let source_peer = hub_source_session.peer().peer_id;
    let downstream_peer = hub_downstream_session.peer().peer_id;
    let block = founder_block(&config);
    let block_item = InventoryItem {
        kind: InventoryKind::Block,
        hash: block.header.block_id(),
    };

    hub_source_session
        .expect(RequestKey::Object(block_item))
        .unwrap();
    assert!(
        source_session
            .send(&Message::Block(block.clone()), QueueClass::RequiredData)
            .await
            .unwrap()
    );
    let received_block = match hub_source_session.read_event().await.unwrap() {
        PeerEvent::MatchedResponse {
            peer_id,
            key: RequestKey::Object(item),
            message: Message::Block(received),
        } => {
            assert_eq!(peer_id, source_peer);
            assert_eq!(item, block_item);
            received
        }
        other => panic!("expected matched block response, got {other:?}"),
    };

    let submission = node.submit_block(received_block).await.unwrap();
    assert!(matches!(submission.result, Ok(AcceptOutcome::Extended)));
    let authorization = submission
        .relay_authorization
        .expect("accepted block must authorize relay");
    let commands = node.relay_inventory_commands(
        Some(source_peer),
        [source_peer, downstream_peer],
        authorization,
    );
    assert_eq!(commands.len(), 1);
    let PeerCommand::Send {
        peer_id,
        message,
        class,
    } = commands.into_iter().next().unwrap()
    else {
        panic!("validated block relay must emit Send");
    };
    assert_eq!(peer_id, downstream_peer);
    assert_eq!(message, Message::Inv(vec![block_item]));
    assert!(hub_downstream_session.send(&message, class).await.unwrap());
    match downstream_session.read_event().await.unwrap() {
        PeerEvent::Message {
            message: Message::Inv(items),
            ..
        } => assert_eq!(items, vec![block_item]),
        other => panic!("expected downstream block inventory, got {other:?}"),
    }

    let invalid = unknown_parent_block(&config);
    let invalid_item = InventoryItem {
        kind: InventoryKind::Block,
        hash: invalid.header.block_id(),
    };
    hub_source_session
        .expect(RequestKey::Object(invalid_item))
        .unwrap();
    assert!(
        source_session
            .send(&Message::Block(invalid), QueueClass::RequiredData)
            .await
            .unwrap()
    );
    let invalid_received = match hub_source_session.read_event().await.unwrap() {
        PeerEvent::MatchedResponse {
            key: RequestKey::Object(item),
            message: Message::Block(received),
            ..
        } => {
            assert_eq!(item, invalid_item);
            received
        }
        other => panic!("expected matched invalid block response, got {other:?}"),
    };
    let rejected = node.submit_block(invalid_received).await.unwrap();
    assert!(rejected.result.is_err());
    assert!(rejected.relay_authorization.is_none());
    assert!(
        timeout(Duration::from_millis(50), downstream_session.read_event())
            .await
            .is_err(),
        "invalid block produced downstream traffic"
    );
}

#[tokio::test]
async fn admitted_transaction_relays_over_tcp_but_conflicting_spend_does_not() {
    let dir = TestDir::scoped("relay", "transaction");
    let config = chain_config();
    let chain_id = config.anchor_header.block_id();
    let (state, spendable) = state_with_spendable_utxo(&dir, &config);
    let mut node = OregonNode::new(
        state,
        MempoolConfig::default(),
        AcceptAllSpends,
        TcpTransport,
    )
    .await
    .unwrap();

    let hub = NodeNetwork::bind(
        TcpTransport,
        node_config(),
        hello(chain_id, 0x50),
        magic(chain_id),
    )
    .await
    .unwrap();
    let source = NodeNetwork::bind(
        TcpTransport,
        node_config(),
        hello(chain_id, 0x40),
        magic(chain_id),
    )
    .await
    .unwrap();
    let downstream = NodeNetwork::bind(
        TcpTransport,
        node_config(),
        hello(chain_id, 0x60),
        magic(chain_id),
    )
    .await
    .unwrap();

    let hub_addr = hub.local_addr();
    let (source_outcome, hub_source_outcome) =
        tokio::join!(source.connect(hub_addr), hub.accept());
    let (downstream_outcome, hub_downstream_outcome) =
        tokio::join!(downstream.connect(hub_addr), hub.accept());
    let mut source_session = source_outcome.unwrap().session;
    let mut hub_source_session = hub_source_outcome.unwrap().session;
    let mut downstream_session = downstream_outcome.unwrap().session;
    let mut hub_downstream_session = hub_downstream_outcome.unwrap().session;

    let source_peer = hub_source_session.peer().peer_id;
    let downstream_peer = hub_downstream_session.peer().peer_id;
    let transaction = spend(spendable, 90_000, 0x52);
    let tx_item = InventoryItem {
        kind: InventoryKind::Transaction,
        hash: transaction.txid(),
    };

    hub_source_session.expect(RequestKey::Object(tx_item)).unwrap();
    assert!(
        source_session
            .send(
                &Message::Transaction(transaction.clone()),
                QueueClass::RequiredData,
            )
            .await
            .unwrap()
    );
    let received_transaction = match hub_source_session.read_event().await.unwrap() {
        PeerEvent::MatchedResponse {
            peer_id,
            key: RequestKey::Object(item),
            message: Message::Transaction(received),
        } => {
            assert_eq!(peer_id, source_peer);
            assert_eq!(item, tx_item);
            received
        }
        other => panic!("expected matched transaction response, got {other:?}"),
    };

    let submission = node.submit_transaction(received_transaction).await.unwrap();
    assert!(submission.result.is_ok());
    let authorization = submission
        .relay_authorization
        .expect("admitted transaction must authorize relay");
    let commands = node.relay_inventory_commands(
        Some(source_peer),
        [source_peer, downstream_peer],
        authorization,
    );
    assert_eq!(commands.len(), 1);
    let PeerCommand::Send {
        peer_id,
        message,
        class,
    } = commands.into_iter().next().unwrap()
    else {
        panic!("validated transaction relay must emit Send");
    };
    assert_eq!(peer_id, downstream_peer);
    assert_eq!(message, Message::Inv(vec![tx_item]));
    assert!(hub_downstream_session.send(&message, class).await.unwrap());
    match downstream_session.read_event().await.unwrap() {
        PeerEvent::Message {
            message: Message::Inv(items),
            ..
        } => assert_eq!(items, vec![tx_item]),
        other => panic!("expected downstream transaction inventory, got {other:?}"),
    }

    let conflict = spend(spendable, 80_000, 0x53);
    let conflict_item = InventoryItem {
        kind: InventoryKind::Transaction,
        hash: conflict.txid(),
    };
    assert_ne!(conflict_item, tx_item);
    hub_source_session
        .expect(RequestKey::Object(conflict_item))
        .unwrap();
    assert!(
        source_session
            .send(&Message::Transaction(conflict), QueueClass::RequiredData)
            .await
            .unwrap()
    );
    let conflicting_received = match hub_source_session.read_event().await.unwrap() {
        PeerEvent::MatchedResponse {
            key: RequestKey::Object(item),
            message: Message::Transaction(received),
            ..
        } => {
            assert_eq!(item, conflict_item);
            received
        }
        other => panic!("expected matched conflicting transaction, got {other:?}"),
    };
    let rejected = node.submit_transaction(conflicting_received).await.unwrap();
    assert!(rejected.result.is_err());
    assert!(rejected.relay_authorization.is_none());
    assert!(
        timeout(Duration::from_millis(50), downstream_session.read_event())
            .await
            .is_err(),
        "policy-rejected transaction produced downstream traffic"
    );
}
