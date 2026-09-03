use num_bigint::BigUint;

use crate::Target;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChainWork(BigUint);

impl ChainWork {
    pub fn zero() -> Self {
        Self(BigUint::from(0u8))
    }

    pub fn to_biguint(&self) -> BigUint {
        self.0.clone()
    }

    pub fn add_assign(&mut self, rhs: &Self) {
        self.0 += &rhs.0;
    }
}

pub fn block_work(target: Target) -> ChainWork {
    let numerator = BigUint::from(1u8) << 256usize;
    let denominator = target.to_biguint() + BigUint::from(1u8);
    ChainWork(numerator / denominator)
}

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

    #[test]
    fn chainwork_canonical_storage_bytes_round_trip() {
        let zero = ChainWork::zero();
        assert_eq!(zero.to_canonical_be_bytes(), vec![0]);
        assert_eq!(ChainWork::from_canonical_be_bytes(&[0]).unwrap(), zero);

        let work = block_work(Target::from_biguint(&BigUint::from(1u8)).unwrap());
        let encoded = work.to_canonical_be_bytes();
        assert_eq!(ChainWork::from_canonical_be_bytes(&encoded).unwrap(), work);
        assert_eq!(ChainWork::from_canonical_be_bytes(&[]), None);
        assert_eq!(ChainWork::from_canonical_be_bytes(&[0, 1]), None);
    }
}
