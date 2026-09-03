use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConsensusError {
    #[error("target must be non-zero")]
    ZeroTarget,
    #[error("target exceeds 256 bits")]
    TargetExceeds256Bits,
    #[error("target exceeds POW_LIMIT")]
    TargetAbovePowLimit,
    #[error("INITIAL_TARGET exceeds POW_LIMIT")]
    InitialTargetAbovePowLimit,
    #[error("consensus arithmetic overflow")]
    ArithmeticOverflow,
    #[error("invalid non-genesis height")]
    InvalidHeight,
    #[error("unexpected difficulty target")]
    UnexpectedTarget,
    #[error("invalid median-time-past window")]
    InvalidMtpWindow,
    #[error("block timestamp is not greater than median-time-past")]
    TimestampNotAfterMtp,
    #[error("previous block id does not match parent")]
    PreviousBlockMismatch,
    #[error("RandomX key-block height does not match the Oregon schedule")]
    PowKeyHeightMismatch,
    #[error("RandomX engine is bound to the wrong epoch key")]
    PowEngineKeyMismatch,
    #[error("RandomX hash does not meet the committed target")]
    InsufficientProofOfWork,
    #[error("coinbase structure is invalid")]
    InvalidCoinbase,
    #[error("height-1 founder output is invalid")]
    InvalidFounderOutput,
    #[error("coinbase claims more than subsidy plus fees")]
    CoinbaseOverClaim,
    #[error("block exceeds v1 canonical byte limit")]
    BlockTooLarge,
    #[error("transaction {0} exceeds v1 canonical byte limit")]
    TransactionTooLarge(usize),
    #[error("non-genesis block has no transactions")]
    EmptyNonGenesisBlock,
    #[error("transaction root does not match header")]
    MerkleRootMismatch,
    #[error("normal transaction uses null outpoint")]
    NullOutpointInNormalTransaction,
    #[error("multiple coinbase-form transactions")]
    MultipleCoinbase,
}
