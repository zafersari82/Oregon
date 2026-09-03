use num_bigint::BigUint;

use crate::{
    ConsensusError, ConsensusParams, Target,
    params::{ASERT_HALF_LIFE_SECONDS, ASERT_RADIX, TARGET_BLOCK_SECONDS},
};

pub fn required_target(
    height: u64,
    parent_timestamp: u64,
    genesis_timestamp: u64,
    params: &ConsensusParams,
) -> Result<Target, ConsensusError> {
    if height == 0 {
        return Err(ConsensusError::InvalidHeight);
    }
    if height == 1 {
        return Ok(params.initial_target);
    }

    let time_delta = i128::from(parent_timestamp) - i128::from(genesis_timestamp);
    let height_delta = i128::from(height - 2);
    let ideal = i128::from(TARGET_BLOCK_SECONDS)
        .checked_mul(
            height_delta
                .checked_add(1)
                .ok_or(ConsensusError::ArithmeticOverflow)?,
        )
        .ok_or(ConsensusError::ArithmeticOverflow)?;
    let drift = time_delta
        .checked_sub(ideal)
        .ok_or(ConsensusError::ArithmeticOverflow)?;
    let exponent = drift
        .checked_mul(ASERT_RADIX)
        .ok_or(ConsensusError::ArithmeticOverflow)?
        / ASERT_HALF_LIFE_SECONDS;

    let num_shifts = exponent >> 16;
    let frac = exponent
        .checked_sub(
            num_shifts
                .checked_mul(ASERT_RADIX)
                .ok_or(ConsensusError::ArithmeticOverflow)?,
        )
        .ok_or(ConsensusError::ArithmeticOverflow)?;
    if !(0..ASERT_RADIX).contains(&frac) {
        return Err(ConsensusError::ArithmeticOverflow);
    }

    let term1 = 195_766_423_245_049i128
        .checked_mul(frac)
        .ok_or(ConsensusError::ArithmeticOverflow)?;
    let frac2 = frac
        .checked_mul(frac)
        .ok_or(ConsensusError::ArithmeticOverflow)?;
    let frac3 = frac2
        .checked_mul(frac)
        .ok_or(ConsensusError::ArithmeticOverflow)?;
    let term2 = 971_821_376i128
        .checked_mul(frac2)
        .ok_or(ConsensusError::ArithmeticOverflow)?;
    let term3 = 5_127i128
        .checked_mul(frac3)
        .ok_or(ConsensusError::ArithmeticOverflow)?;
    let polynomial = term1
        .checked_add(term2)
        .and_then(|value| value.checked_add(term3))
        .and_then(|value| value.checked_add(1i128 << 47))
        .ok_or(ConsensusError::ArithmeticOverflow)?;
    let factor = u64::try_from((polynomial >> 48) + ASERT_RADIX)
        .map_err(|_| ConsensusError::ArithmeticOverflow)?;

    if num_shifts >= 256 {
        return Ok(params.pow_limit);
    }
    if num_shifts <= -257 {
        return Target::from_biguint(&BigUint::from(1u8));
    }

    let mut candidate = params.initial_target.to_biguint() * BigUint::from(factor);
    if num_shifts < 0 {
        candidate >>= usize::try_from(-num_shifts).map_err(|_| ConsensusError::ArithmeticOverflow)?;
    } else {
        candidate <<= usize::try_from(num_shifts).map_err(|_| ConsensusError::ArithmeticOverflow)?;
    }
    candidate >>= 16usize;

    if candidate == BigUint::from(0u8) {
        return Target::from_biguint(&BigUint::from(1u8));
    }
    if candidate > params.pow_limit.to_biguint() {
        return Ok(params.pow_limit);
    }

    Target::from_biguint(&candidate)
}

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;
    use proptest::prelude::*;

    use super::*;
    use crate::{ConsensusParams, Target};

    const G: u64 = 1_800_000_000;

    fn target(value: u64) -> Target {
        Target::from_biguint(&BigUint::from(value)).unwrap()
    }

    fn params() -> ConsensusParams {
        ConsensusParams::new(target(10_000_000), target(1_000_000), [0x42; 32]).unwrap()
    }

    #[test]
    fn h1_is_initial() {
        assert_eq!(
            required_target(1, 0, 0, &params()).unwrap(),
            target(1_000_000)
        );
    }

    #[test]
    fn on_schedule_is_unchanged() {
        assert_eq!(
            required_target(2, G + 300, G, &params()).unwrap(),
            target(1_000_000)
        );
    }

    #[test]
    fn one_half_life_late_doubles() {
        assert_eq!(
            required_target(2, G + 21_900, G, &params()).unwrap(),
            target(2_000_000)
        );
    }

    #[test]
    fn one_half_life_early_halves() {
        assert_eq!(
            required_target(2, G - 21_300, G, &params()).unwrap(),
            target(500_000)
        );
    }

    #[test]
    fn half_half_life_late_is_frozen() {
        assert_eq!(
            required_target(2, G + 11_100, G, &params()).unwrap(),
            target(1_414_093)
        );
    }

    #[test]
    fn huge_positive_exponent_clamps_to_pow_limit() {
        assert_eq!(
            required_target(2, u64::MAX, G, &params()).unwrap(),
            target(10_000_000)
        );
    }

    #[test]
    fn huge_negative_exponent_clamps_to_one() {
        assert_eq!(
            required_target(2, 0, G, &params()).unwrap(),
            target(1)
        );
    }

    proptest! {
        #[test]
        fn on_schedule_keeps_initial_target(height in 2u64..100_000) {
            let parent = G + 300 * (height - 1);
            prop_assert_eq!(
                required_target(height, parent, G, &params()).unwrap(),
                target(1_000_000)
            );
        }

        #[test]
        fn every_bounded_result_is_nonzero_and_within_pow_limit(
            height in 1u64..100_000,
            parent_timestamp in 0u64..3_600_000_000u64,
        ) {
            let p = params();
            let result = required_target(height, parent_timestamp, G, &p).unwrap();
            let numeric = result.to_biguint();
            prop_assert!(numeric > BigUint::from(0u8));
            prop_assert!(numeric <= p.pow_limit.to_biguint());
        }
    }
}
