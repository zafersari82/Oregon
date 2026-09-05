#![forbid(unsafe_code)]

mod error;
mod io;
mod tcp;
mod transport;

pub use error::NetworkError;
pub use io::FramedConnection;
pub use tcp::{TcpConnection, TcpTransport, TcpTransportListener};
pub use transport::{Transport, TransportConnection, TransportListener};

#[cfg(test)]
mod tests;
