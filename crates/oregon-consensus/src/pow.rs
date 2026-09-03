use oregon_pow::{LightEngine, derive_randomx_key, hash_meets_target, key_block_height, pow_input};
use oregon_primitives::{BlockHeader, Hash256};

use crate::{ConsensusError, Target};

/// Supplies block IDs from an already-validated active chain.
///
/// The PoW validator chooses the required RandomX key-block height itself. Implementations must
/// return the block ID committed at that height by the validated chain, never a miner-provided ID.
pub trait PowKeyBlockSource {
    fn validated_block_id_at_height(&self, height: u64) -> Option<Hash256>;
}

pub fn validate_header_pow<S: PowKeyBlockSource + ?Sized>(
    header: &BlockHeader,
    candidate_height: u64,
    key_blocks: &S,
    engine: &mut LightEngine,
) -> Result<[u8; 32], ConsensusError> {
    let required_key_height = key_block_height(candidate_height);
    let key_block_id = key_blocks
        .validated_block_id_at_height(required_key_height)
        .ok_or(ConsensusError::PowKeyBlockUnavailable)?;

    let expected_key = derive_randomx_key(key_block_id);
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
