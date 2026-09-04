use oregon_primitives::PrimitiveError;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProtocolError {
    #[error("unknown protocol message type {0:#04x}")]
    UnknownMessageType(u8),
    #[error("unknown inventory kind {0:#04x}")]
    UnknownInventoryKind(u8),
    #[error("protocol list contains {actual} items, maximum is {max}")]
    ListLimitExceeded { actual: usize, max: usize },
    #[error("frame payload is {actual} bytes, maximum is {max}")]
    PayloadTooLarge { actual: usize, max: usize },
    #[error("unsupported frame version {0}")]
    UnsupportedFrameVersion(u8),
    #[error("protocol-v1 frame flags must be zero, got {0:#06x}")]
    NonZeroFlags(u16),
    #[error("frame network magic does not match the configured chain")]
    WrongNetworkMagic,
    #[error("frame declared {declared} payload bytes but received {actual}")]
    PayloadLengthMismatch { declared: u32, actual: usize },
    #[error("frame checksum mismatch")]
    ChecksumMismatch,
    #[error("GetHeaders stop-present flag must be zero or one, got {0}")]
    InvalidStopFlag(u8),
    #[error("protocol version range is invalid: minimum {min}, maximum {max}")]
    InvalidProtocolVersionRange { min: u16, max: u16 },
    #[error("peers have no common protocol version")]
    NoCommonProtocolVersion,
    #[error("required feature bits are not included in offered features: {0:#018x}")]
    RequiredFeaturesNotOffered(u64),
    #[error("required feature bits are unknown or unsupported: {0:#018x}")]
    UnsupportedRequiredFeatures(u64),
    #[error(transparent)]
    Primitive(#[from] PrimitiveError),
}
