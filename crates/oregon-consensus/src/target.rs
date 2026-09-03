#[cfg(test)]
mod tests {
    use num_bigint::BigUint;
    use super::*;

    #[test]
    fn zero_target_is_invalid() {
        assert_eq!(Target::from_le_bytes([0; 32]), Err(ConsensusError::ZeroTarget));
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
        assert_eq!(Target::from_biguint(&value), Err(ConsensusError::TargetExceeds256Bits));
    }
}
