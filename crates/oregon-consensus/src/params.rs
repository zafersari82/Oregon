use crate::{ConsensusError, Target};

pub const TARGET_BLOCK_SECONDS: u64 = 300;
pub const ASERT_HALF_LIFE_SECONDS: i128 = 21_600;
pub const ASERT_RADIX: i128 = 65_536;
pub const HALVING_INTERVAL: u64 = 200_000;
pub const INITIAL_SUBSIDY_BASE_UNITS: u64 = 237_500_000;
pub const MAX_BLOCK_BYTES: usize = 1_048_576;
pub const MAX_TRANSACTION_BYTES: usize = 102_400;
pub const KEY_COMMIT_V1: u8 = 0x01;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsensusParams {
    pub pow_limit: Target,
    pub initial_target: Target,
    pub founder_key_commitment: [u8; 32],
}

impl ConsensusParams {
    pub fn new(
        pow_limit: Target,
        initial_target: Target,
        founder_key_commitment: [u8; 32],
    ) -> Result<Self, ConsensusError> {
        if initial_target.to_biguint() > pow_limit.to_biguint() {
            return Err(ConsensusError::InitialTargetAbovePowLimit);
        }

        Ok(Self {
            pow_limit,
            initial_target,
            founder_key_commitment,
        })
    }
}

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;

    use super::*;

    #[test]
    fn initial_target_cannot_exceed_pow_limit() {
        let pow_limit = Target::from_biguint(&BigUint::from(100u32)).unwrap();
        let initial = Target::from_biguint(&BigUint::from(101u32)).unwrap();
        assert_eq!(
            ConsensusParams::new(pow_limit, initial, [7u8; 32]),
            Err(ConsensusError::InitialTargetAbovePowLimit)
        );
    }
}
