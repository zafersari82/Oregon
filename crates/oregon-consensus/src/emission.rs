use oregon_primitives::{Amount, FOUNDER_ALLOCATION_BASE_UNITS};

use crate::{
    ConsensusError,
    params::{HALVING_INTERVAL, INITIAL_SUBSIDY_BASE_UNITS},
};

pub const SCHEDULED_MINING_ISSUANCE_BASE_UNITS: u64 = 94_999_997_000_000;
pub const SCHEDULED_TOTAL_WITH_FOUNDER_BASE_UNITS: u64 =
    SCHEDULED_MINING_ISSUANCE_BASE_UNITS + FOUNDER_ALLOCATION_BASE_UNITS;

pub fn block_subsidy(height: u64) -> Result<Amount, ConsensusError> {
    if height == 0 {
        return Amount::from_base_units(0).map_err(|_| ConsensusError::ArithmeticOverflow);
    }

    let era = (height - 1) / HALVING_INTERVAL;
    let subsidy = if era >= 64 {
        0
    } else {
        INITIAL_SUBSIDY_BASE_UNITS >> era as u32
    };

    Amount::from_base_units(subsidy).map_err(|_| ConsensusError::ArithmeticOverflow)
}

#[cfg(test)]
mod tests {
    use oregon_primitives::MAX_SUPPLY_BASE_UNITS;

    use super::*;
    use crate::params::HALVING_INTERVAL;

    #[test]
    fn genesis_subsidy_is_zero() {
        assert_eq!(block_subsidy(0).unwrap().base_units(), 0);
    }

    #[test]
    fn exact_halving_boundaries() {
        assert_eq!(block_subsidy(1).unwrap().base_units(), 237_500_000);
        assert_eq!(block_subsidy(200_000).unwrap().base_units(), 237_500_000);
        assert_eq!(block_subsidy(200_001).unwrap().base_units(), 118_750_000);
    }

    #[test]
    fn era_27_is_last_positive_era() {
        assert_eq!(
            block_subsidy(27 * HALVING_INTERVAL + 1)
                .unwrap()
                .base_units(),
            1
        );
        assert_eq!(
            block_subsidy(28 * HALVING_INTERVAL + 1)
                .unwrap()
                .base_units(),
            0
        );
    }

    #[test]
    fn scheduled_issuance_is_exact() {
        let mut total = 0u128;
        for era in 0..28u64 {
            total += u128::from(
                block_subsidy(era * HALVING_INTERVAL + 1)
                    .unwrap()
                    .base_units(),
            ) * u128::from(HALVING_INTERVAL);
        }
        assert_eq!(total, 94_999_997_000_000);
        assert_eq!(
            MAX_SUPPLY_BASE_UNITS - SCHEDULED_TOTAL_WITH_FOUNDER_BASE_UNITS,
            3_000_000
        );
    }
}
