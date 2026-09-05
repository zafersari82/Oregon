use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use oregon_mempool::MempoolConfig;
use oregon_network::TcpTransport;
use oregon_node::{NodeConfig, NodeNetwork};
use oregon_peer::{Direction, PeerConfig};
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
