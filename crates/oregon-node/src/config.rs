use std::net::SocketAddr;

use oregon_mempool::MempoolConfig;
use oregon_peer::PeerConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeConfig {
    pub listen_addr: SocketAddr,
    pub bootstrap_peers: Vec<SocketAddr>,
    pub peer: PeerConfig,
    pub mempool: MempoolConfig,
}
