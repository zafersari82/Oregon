use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PrimitiveError {
    #[error("amount arithmetic overflow")]
    AmountOverflow,
    #[error("amount arithmetic underflow")]
    AmountUnderflow,
    #[error("amount exceeds Oregon maximum supply")]
    AmountAboveMaximum,
    #[error("unexpected end of input")]
    UnexpectedEof,
    #[error("non-canonical varint")]
    NonCanonicalVarInt,
    #[error("decoded length exceeds configured limit")]
    LengthLimitExceeded,
    #[error("invalid protocol version {0}")]
    InvalidVersion(u16),
    #[error("trailing bytes after complete consensus object")]
    TrailingBytes,
    #[error("invalid fixed-size hash length: expected 32, got {0}")]
    InvalidHashLength(usize),
    #[error("invalid lowercase hexadecimal hash")]
    InvalidHashHex,
    #[error("block transaction list must not be empty")]
    EmptyBlockTransactions,
}
