use std::{net::SocketAddr, time::Duration};

use oregon_protocol::{
    FRAME_HEADER_BYTES, FrameHeader, Message, ProtocolError, build_frame_header, decode_message,
    encode_message, verify_frame_payload,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::{Instant, sleep_until, timeout};

use crate::{NetworkError, TransportConnection};

const READ_NO_PROGRESS_TIMEOUT: Duration = Duration::from_secs(15);
const READ_ABSOLUTE_DEADLINE: Duration = Duration::from_secs(60);
const WRITE_ABSOLUTE_DEADLINE: Duration = Duration::from_secs(15);

pub struct FramedConnection<S> {
    stream: S,
    remote_addr: SocketAddr,
    magic: [u8; 4],
}

impl<S> FramedConnection<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    pub fn new(stream: S, remote_addr: SocketAddr, magic: [u8; 4]) -> Self {
        Self {
            stream,
            remote_addr,
            magic,
        }
    }

    pub async fn read_message(&mut self) -> Result<Message, NetworkError> {
        let deadline = Instant::now() + READ_ABSOLUTE_DEADLINE;
        let mut bytes = [0u8; FRAME_HEADER_BYTES];
        read_exact_counted(&mut self.stream, &mut bytes, deadline).await?;

        let declared_payload_length = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let mut payload = vec![0u8; declared_payload_length as usize];
        read_exact_counted(&mut self.stream, &mut payload, deadline).await?;

        let header = match FrameHeader::decode(&bytes) {
            Err(ProtocolError::PayloadTooLarge { actual, max }) => {
                return Err(NetworkError::OversizedFrame {
                    declared: actual as u32,
                    max,
                });
            }
            Err(error) => return Err(error.into()),
            Ok(header) => header,
        };
        if header.network_magic != self.magic {
            return Err(ProtocolError::WrongNetworkMagic.into());
        }
        verify_frame_payload(&header, self.magic, &payload)?;
        Ok(decode_message(header.message_type, &payload)?)
    }

    pub async fn write_message(&mut self, message: &Message) -> Result<(), NetworkError> {
        let (message_type, payload) = encode_message(message)?;
        let header = build_frame_header(self.magic, message_type, &payload)?;
        timeout(WRITE_ABSOLUTE_DEADLINE, async {
            self.stream.write_all(&header.encode()).await?;
            self.stream.write_all(&payload).await?;
            self.stream.flush().await
        })
        .await
        .map_err(|_| NetworkError::WriteDeadlineExceeded)??;
        Ok(())
    }

    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }

    pub async fn shutdown(&mut self) -> Result<(), NetworkError> {
        self.stream.shutdown().await?;
        Ok(())
    }
}

async fn read_exact_counted<R>(
    reader: &mut R,
    buffer: &mut [u8],
    deadline: Instant,
) -> Result<(), NetworkError>
where
    R: AsyncRead + Unpin,
{
    let mut received = 0;
    while received < buffer.len() {
        let count = tokio::select! {
            biased;
            _ = sleep_until(deadline) => return Err(NetworkError::ReadDeadlineExceeded),
            result = timeout(
                READ_NO_PROGRESS_TIMEOUT,
                reader.read(&mut buffer[received..]),
            ) => result.map_err(|_| NetworkError::ReadNoProgressTimeout)??,
        };
        if count == 0 {
            return Err(NetworkError::TruncatedFrame {
                received,
                expected: buffer.len(),
            });
        }
        received += count;
    }
    Ok(())
}

#[async_trait::async_trait]
impl<S> TransportConnection for FramedConnection<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    fn remote_addr(&self) -> SocketAddr {
        FramedConnection::remote_addr(self)
    }

    async fn read_message(&mut self) -> Result<Message, NetworkError> {
        FramedConnection::read_message(self).await
    }

    async fn write_message(&mut self, message: &Message) -> Result<(), NetworkError> {
        FramedConnection::write_message(self, message).await
    }

    async fn shutdown(&mut self) -> Result<(), NetworkError> {
        FramedConnection::shutdown(self).await
    }
}
