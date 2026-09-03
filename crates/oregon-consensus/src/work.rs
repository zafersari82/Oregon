//! RED phase: exact chain-work consensus tests.

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;

    use super::*;
    use crate::Target;

    #[test]
    fn max_target_has_one_work_unit() {
        let target = Target::from_le_bytes([0xff; 32]).unwrap();
        assert_eq!(block_work(target).to_biguint(), BigUint::from(1u8));
    }

    #[test]
    fn target_one_has_two_to_255_work() {
        let target = Target::from_biguint(&BigUint::from(1u8)).unwrap();
        assert_eq!(
            block_work(target).to_biguint(),
            BigUint::from(1u8) << 255usize
        );
    }
}
