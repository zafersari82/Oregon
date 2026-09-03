//! RED phase: deterministic and property tests for Oregon's exact ASERT rule.

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
