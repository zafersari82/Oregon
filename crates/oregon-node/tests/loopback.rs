use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use oregon_mempool::MempoolConfig;
use oregon_network::TcpTransport;
use oregon_node::{NodeConfig, NodeNetwork, NodeNetworkError};
use oregon_peer::{Direction, PeerConfig, PeerError};
use oregon_primitives::Hash256;
use oregon_protocol::{
    FeatureSet, Hello, PROTOCOL_VERSION_CURRENT, PROTOCOL_VERSION_MIN, network_magic,
};

fn config() -> NodeConfig {
    NodeConfig {
        listen_addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        bootstrap_peers: Vec::new(),
        peer: PeerConfig::default(),
        mempool: MempoolConfig::default(),
    }
}

fn hello(chain_id: Hash256, nonce: u8) -> Hello {
    Hello {
        min_protocol_version: PROTOCOL_VERSION_MIN,
        max_protocol_version: PROTOCOL_VERSION_CURRENT,
        chain_id,
        instance_nonce: [nonce; 16],
        offered_features: FeatureSet::KNOWN,
        required_features: FeatureSet::HEADERS_SYNC,
        best_height: 0,
        best_block_id: chain_id,
    }
}

#[tokio::test]
async fn three_nodes_establish_real_tcp_sessions_with_negotiated_features() {
    let chain_id = Hash256::from_bytes([0x42; 32]);
    let magic = network_magic(chain_id);

    let mut node_b = NodeNetwork::bind(TcpTransport, config(), hello(chain_id, 0x20), magic)
        .await
        .unwrap();
    let node_a = NodeNetwork::bind(TcpTransport, config(), hello(chain_id, 0x10), magic)
        .await
        .unwrap();
    let node_c = NodeNetwork::bind(TcpTransport, config(), hello(chain_id, 0x30), magic)
        .await
        .unwrap();

    let b_addr = node_b.local_addr();

    let (a_outbound, b_from_a) = tokio::join!(node_a.connect(b_addr), node_b.accept());
    let a_outbound = a_outbound.unwrap();
    let b_from_a = b_from_a.unwrap();

    let (c_outbound, b_from_c) = tokio::join!(node_c.connect(b_addr), node_b.accept());
    let c_outbound = c_outbound.unwrap();
    let b_from_c = b_from_c.unwrap();

    assert_eq!(a_outbound.session.peer().direction, Direction::Outbound);
    assert_eq!(c_outbound.session.peer().direction, Direction::Outbound);
    assert_eq!(b_from_a.session.peer().direction, Direction::Inbound);
    assert_eq!(b_from_c.session.peer().direction, Direction::Inbound);

    for peer in [
        a_outbound.session.peer(),
        c_outbound.session.peer(),
        b_from_a.session.peer(),
        b_from_c.session.peer(),
    ] {
        assert_eq!(peer.negotiated_version, PROTOCOL_VERSION_CURRENT);
        assert_eq!(peer.features, FeatureSet::KNOWN);
    }
}

#[tokio::test]
async fn real_tcp_self_connection_is_rejected_by_process_nonce() {
    let chain_id = Hash256::from_bytes([0x43; 32]);
    let magic = network_magic(chain_id);
    let mut node_b = NodeNetwork::bind(TcpTransport, config(), hello(chain_id, 0x55), magic)
        .await
        .unwrap();
    let node_a = NodeNetwork::bind(TcpTransport, config(), hello(chain_id, 0x55), magic)
        .await
        .unwrap();

    let (outbound, inbound) = tokio::join!(node_a.connect(node_b.local_addr()), node_b.accept());

    assert!(matches!(
        outbound,
        Err(NodeNetworkError::Peer(PeerError::SelfPeer))
    ));
    assert!(matches!(
        inbound,
        Err(NodeNetworkError::Peer(PeerError::SelfPeer))
    ));
}

#[tokio::test]
async fn simultaneous_duplicate_tcp_dials_choose_the_same_physical_direction() {
    let chain_id = Hash256::from_bytes([0x44; 32]);
    let magic = network_magic(chain_id);
    let mut node_a = NodeNetwork::bind(TcpTransport, config(), hello(chain_id, 0x10), magic)
        .await
        .unwrap();
    let mut node_b = NodeNetwork::bind(TcpTransport, config(), hello(chain_id, 0x20), magic)
        .await
        .unwrap();
    let a_addr = node_a.local_addr();
    let b_addr = node_b.local_addr();

    let (a_connects_b, b_accepts_a, b_connects_a, a_accepts_b) = tokio::join!(
        node_a.connect(b_addr),
        node_b.accept(),
        node_b.connect(a_addr),
        node_a.accept(),
    );

    let a_preferred = a_connects_b.expect("A must retain outbound when A nonce < B nonce");
    let b_preferred = b_accepts_a.expect("B must retain inbound when B nonce > A nonce");
    assert_eq!(a_preferred.session.peer().direction, Direction::Outbound);
    assert_eq!(b_preferred.session.peer().direction, Direction::Inbound);

    let a_nonpreferred_lost = matches!(
        a_accepts_b,
        Err(NodeNetworkError::Peer(PeerError::DuplicatePeer))
    ) || a_preferred.replaced_peer.is_some();
    let b_nonpreferred_lost = matches!(
        b_connects_a,
        Err(NodeNetworkError::Peer(PeerError::DuplicatePeer))
    ) || b_preferred.replaced_peer.is_some();

    assert!(a_nonpreferred_lost, "A must reject or replace its inbound duplicate");
    assert!(b_nonpreferred_lost, "B must reject or replace its outbound duplicate");
}
