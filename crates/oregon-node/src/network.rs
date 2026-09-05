use std::net::SocketAddr;

use oregon_network::{NetworkError, Transport, TransportListener};
use oregon_peer::{EstablishOutcome, PeerError, PeerService};
use oregon_protocol::Hello;
use thiserror::Error;

use crate::NodeConfig;

#[derive(Debug, Error)]
pub enum NodeNetworkError {
    #[error(transparent)]
    Network(#[from] NetworkError),
    #[error(transparent)]
    Peer(#[from] PeerError),
}

pub struct NodeNetwork<T: Transport> {
    listener: T::Listener,
    peer_service: PeerService<T>,
    magic: [u8; 4],
}

impl<T: Transport> NodeNetwork<T> {
    pub async fn bind(
        transport: T,
        config: NodeConfig,
        local_hello: Hello,
        magic: [u8; 4],
    ) -> Result<Self, NodeNetworkError> {
        let listener = transport.bind(config.listen_addr, magic).await?;
        let peer_service = PeerService::new(transport, config.peer, local_hello)?;
        Ok(Self {
            listener,
            peer_service,
            magic,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.listener.local_addr()
    }

    pub async fn connect(
        &self,
        addr: SocketAddr,
    ) -> Result<EstablishOutcome<T::Connection>, NodeNetworkError> {
        Ok(self.peer_service.connect(addr, self.magic).await?)
    }

    pub async fn accept(&mut self) -> Result<EstablishOutcome<T::Connection>, NodeNetworkError> {
        let connection = self.listener.accept().await?;
        Ok(self.peer_service.accept_connection(connection).await?)
    }
}
