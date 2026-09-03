use num_bigint::BigUint;
use num_traits::Zero;

use crate::ConsensusError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Target([u8; 32]);

impl Target {
    pub fn from_le_bytes(bytes: [u8; 32]) -> Result<Self, ConsensusError> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(ConsensusError::ZeroTarget);
        }
        Ok(Self(bytes))
    }

    pub const fn to_le_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn to_biguint(&self) -> BigUint {
        BigUint::from_bytes_le(&self.0)
    }

    pub fn from_biguint(value: &BigUint) -> Result<Self, ConsensusError> {
        if value.is_zero() {
            return Err(ConsensusError::ZeroTarget);
        }

        let bytes = value.to_bytes_le();
        if bytes.len() > 32 {
            return Err(ConsensusError::TargetExceeds256Bits);
        }

        let mut fixed = [0u8; 32];
        fixed[..bytes.len()].copy_from_slice(&bytes);
        Ok(Self(fixed))
    }

    pub fn validate_against(self, pow_limit: Target) -> Result<(), ConsensusError> {
        if self.to_biguint() > pow_limit.to_biguint() {
            return Err(ConsensusError::TargetAbovePowLimit);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;

    use super::*;

    #[test]
    fn zero_target_is_invalid() {
        assert_eq!(
            Target::from_le_bytes([0; 32]),
            Err(ConsensusError::ZeroTarget)
        );
    }

    #[test]
    fn little_endian_target_round_trips() {
        let mut bytes = [0u8; 32];
        bytes[0] = 0x34;
        bytes[1] = 0x12;
        let target = Target::from_le_bytes(bytes).unwrap();
        assert_eq!(target.to_le_bytes(), bytes);
        assert_eq!(target.to_biguint(), BigUint::from(0x1234u32));
    }

    #[test]
    fn more_than_256_bits_is_rejected() {
        let value = BigUint::from(1u8) << 256usize;
        assert_eq!(
            Target::from_biguint(&value),
            Err(ConsensusError::TargetExceeds256Bits)
        );
    }
}
