use oregon_consensus::ConsensusParams;
use oregon_primitives::BlockHeader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainConfig {
    pub anchor_header: BlockHeader,
    pub genesis_timestamp: u64,
    pub params: ConsensusParams,
}
