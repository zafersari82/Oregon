use oregon_pow::{LightEngine, derive_randomx_key, hash_meets_target, key_block_height, pow_input};
use oregon_primitives::{BlockHeader, Hash256};

use crate::{ConsensusError, Target};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowContext {
    pub candidate_height: u64,
    pub key_block_height: u64,
    pub key_block_id: Hash256,
}

pub fn validate_header_pow(
    header: &BlockHeader,
    context: PowContext,
    engine: &mut LightEngine,
) -> Result<[u8; 32], ConsensusError> {
    if context.key_block_height != key_block_height(context.candidate_height) {
        return Err(ConsensusError::PowKeyHeightMismatch);
    }

    let expected_key = derive_randomx_key(context.key_block_id);
    if engine.key() != expected_key {
        return Err(ConsensusError::PowEngineKeyMismatch);
    }

    let target = Target::from_le_bytes(header.difficulty_commitment)?;
    let hash = engine.hash(&pow_input(header));
    if !hash_meets_target(hash, target.to_le_bytes()) {
        return Err(ConsensusError::InsufficientProofOfWork);
    }

    Ok(hash)
}
