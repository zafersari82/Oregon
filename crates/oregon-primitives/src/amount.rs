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
