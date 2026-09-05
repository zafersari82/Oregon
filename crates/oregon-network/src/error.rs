use oregon_protocol::ProtocolError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("network I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error("frame declares {declared} payload bytes, maximum is {max}")]
    OversizedFrame { declared: u32, max: usize },
    #[error("frame ended after {received} of {expected} bytes")]
    TruncatedFrame { received: usize, expected: usize },
    #[error("frame read made no progress for 15 seconds")]
    ReadNoProgressTimeout,
    #[error("frame read exceeded the 60 second absolute deadline")]
    ReadDeadlineExceeded,
    #[error("frame write exceeded the 15 second absolute deadline")]
    WriteDeadlineExceeded,
}
