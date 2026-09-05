use std::net::SocketAddr;

use oregon_protocol::Message;

use crate::NetworkError;

#[async_trait::async_trait]
pub trait TransportListener: Send + 'static {
    type Connection: TransportConnection;

    fn local_addr(&self) -> SocketAddr;
    async fn accept(&mut self) -> Result<Self::Connection, NetworkError>;
}

#[async_trait::async_trait]
pub trait Transport: Send + Sync + 'static {
    type Connection: TransportConnection;
    type Listener: TransportListener<Connection = Self::Connection>;

    async fn bind(&self, addr: SocketAddr, magic: [u8; 4]) -> Result<Self::Listener, NetworkError>;
    async fn connect(
        &self,
        addr: SocketAddr,
        magic: [u8; 4],
    ) -> Result<Self::Connection, NetworkError>;
}

#[async_trait::async_trait]
pub trait TransportConnection: Send + 'static {
    fn remote_addr(&self) -> SocketAddr;
    async fn read_message(&mut self) -> Result<Message, NetworkError>;
    async fn write_message(&mut self, message: &Message) -> Result<(), NetworkError>;
    async fn shutdown(&mut self) -> Result<(), NetworkError>;
}
