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
