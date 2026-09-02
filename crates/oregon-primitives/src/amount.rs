use crate::PrimitiveError;

pub const BASE_UNITS_PER_OREG: u64 = 100_000_000;
pub const MAX_SUPPLY_BASE_UNITS: u64 = 1_000_000 * BASE_UNITS_PER_OREG;
pub const FOUNDER_ALLOCATION_BASE_UNITS: u64 = 50_000 * BASE_UNITS_PER_OREG;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Amount(u64);

impl Amount {
    pub fn from_base_units(value: u64) -> Result<Self, PrimitiveError> {
        if value > MAX_SUPPLY_BASE_UNITS {
            return Err(PrimitiveError::AmountAboveMaximum);
        }
        Ok(Self(value))
    }

    pub const fn base_units(self) -> u64 {
        self.0
    }

    pub fn checked_add(self, rhs: Self) -> Result<Self, PrimitiveError> {
        let value = self
            .0
            .checked_add(rhs.0)
            .ok_or(PrimitiveError::AmountOverflow)?;
        Self::from_base_units(value)
    }

    pub fn checked_sub(self, rhs: Self) -> Result<Self, PrimitiveError> {
        let value = self
            .0
            .checked_sub(rhs.0)
            .ok_or(PrimitiveError::AmountUnderflow)?;
        Self::from_base_units(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monetary_constants_are_exact() {
        assert_eq!(BASE_UNITS_PER_OREG, 100_000_000);
        assert_eq!(MAX_SUPPLY_BASE_UNITS, 100_000_000_000_000);
        assert_eq!(FOUNDER_ALLOCATION_BASE_UNITS, 5_000_000_000_000);
        assert_eq!(FOUNDER_ALLOCATION_BASE_UNITS * 20, MAX_SUPPLY_BASE_UNITS);
    }

    #[test]
    fn amount_rejects_values_above_supply_envelope() {
        assert!(Amount::from_base_units(MAX_SUPPLY_BASE_UNITS).is_ok());
        assert!(Amount::from_base_units(MAX_SUPPLY_BASE_UNITS + 1).is_err());
    }

    #[test]
    fn amount_checked_add_never_wraps_or_exceeds_supply() {
        let max = Amount::from_base_units(MAX_SUPPLY_BASE_UNITS).unwrap();
        let one = Amount::from_base_units(1).unwrap();
        assert!(max.checked_add(one).is_err());
    }

    #[test]
    fn amount_checked_sub_rejects_underflow() {
        let zero = Amount::from_base_units(0).unwrap();
        let one = Amount::from_base_units(1).unwrap();
        assert!(zero.checked_sub(one).is_err());
    }
}
