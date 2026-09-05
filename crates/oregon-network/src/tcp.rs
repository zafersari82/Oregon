use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpStream};

use crate::{FramedConnection, NetworkError, Transport, TransportListener};

pub type TcpConnection = FramedConnection<TcpStream>;

#[derive(Debug, Clone, Copy, Default)]
pub struct TcpTransport;

pub struct TcpTransportListener {
    listener: TcpListener,
    local_addr: SocketAddr,
    magic: [u8; 4],
}

#[async_trait::async_trait]
impl TransportListener for TcpTransportListener {
    type Connection = TcpConnection;

    fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    async fn accept(&mut self) -> Result<Self::Connection, NetworkError> {
        let (stream, remote_addr) = self.listener.accept().await?;
        stream.set_nodelay(true)?;
        Ok(FramedConnection::new(stream, remote_addr, self.magic))
    }
}

#[async_trait::async_trait]
impl Transport for TcpTransport {
    type Connection = TcpConnection;
    type Listener = TcpTransportListener;

    async fn bind(&self, addr: SocketAddr, magic: [u8; 4]) -> Result<Self::Listener, NetworkError> {
        let listener = TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;
        Ok(TcpTransportListener {
            listener,
            local_addr,
            magic,
        })
    }

    async fn connect(
        &self,
        addr: SocketAddr,
        magic: [u8; 4],
    ) -> Result<Self::Connection, NetworkError> {
        let stream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true)?;
        let remote_addr = stream.peer_addr()?;
        Ok(FramedConnection::new(stream, remote_addr, magic))
    }
}
