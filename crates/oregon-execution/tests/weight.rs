use oregon_execution::{MeterScheduleV1, ResourceDomain, ResourceError, WeightMeter, WeightRatio};
use proptest::prelude::*;

fn unit_schedule() -> MeterScheduleV1 {
    let unit = WeightRatio::new(1, 1).unwrap();
    MeterScheduleV1::new(1, unit, unit, unit).unwrap()
}

#[test]
fn conversion_rounds_up() {
    let ratio = WeightRatio::new(2, 3).unwrap();
    for (units, want) in [(0, 0), (1, 1), (2, 2), (3, 2)] {
        assert_eq!(ratio.normalized_weight(units), Ok(want));
    }
}

#[test]
fn conversion_uses_wide_products_and_rejects_narrowing_overflow() {
    let ratio = WeightRatio::new(2, 3).unwrap();
    assert_eq!(
        ratio.normalized_weight(u64::MAX),
        Ok(12_297_829_382_473_034_410)
    );
    let identity_at_the_limit = WeightRatio::new(u64::MAX, u64::MAX).unwrap();
    assert_eq!(
        identity_at_the_limit.normalized_weight(u64::MAX),
        Ok(u64::MAX)
    );
    let overflowing = WeightRatio::new(u64::MAX, 1).unwrap();
    assert_eq!(
        overflowing.normalized_weight(2),
        Err(ResourceError::WeightOverflow)
    );
}

#[test]
fn invalid_ratios_and_schedule_versions_fail_closed() {
    assert_eq!(WeightRatio::new(0, 1), Err(ResourceError::InvalidRatio));
    assert_eq!(WeightRatio::new(1, 0), Err(ResourceError::InvalidRatio));
    let unit = WeightRatio::new(1, 1).unwrap();
    assert_eq!(
        MeterScheduleV1::new(0, unit, unit, unit),
        Err(ResourceError::UnsupportedVersion(0))
    );
    assert_eq!(
        MeterScheduleV1::new(2, unit, unit, unit),
        Err(ResourceError::UnsupportedVersion(2))
    );
}

#[test]
fn split_charges_share_cumulative_rounding() {
    let ratio = WeightRatio::new(2, 3).unwrap();
    let schedule = MeterScheduleV1::new(1, ratio, ratio, ratio).unwrap();
    let mut split = WeightMeter::new(schedule, 10, 0).unwrap();
    for _ in 0..3 {
        split.charge(ResourceDomain::Evm, 1).unwrap();
    }
    let mut combined = WeightMeter::new(schedule, 10, 0).unwrap();
    combined.charge(ResourceDomain::Evm, 3).unwrap();
    assert_eq!(split.consumed(), 2);
    assert_eq!(split.consumed(), combined.consumed());
}

#[test]
fn common_work_consumes_budget() {
    let mut meter = WeightMeter::new(unit_schedule(), 7, 2).unwrap();
    meter.charge_common(3).unwrap();
    assert_eq!(meter.consumed(), 5);
    assert_eq!(meter.remaining(), 2);
}

#[test]
fn exhaustion_is_sticky() {
    let ratio = WeightRatio::new(2, 3).unwrap();
    let schedule = MeterScheduleV1::new(1, ratio, ratio, ratio).unwrap();
    let mut meter = WeightMeter::new(schedule, 4, 2).unwrap();
    for _ in 0..3 {
        meter.charge(ResourceDomain::Evm, 1).unwrap();
    }
    assert_eq!(meter.consumed(), 4);
    assert_eq!(meter.charge_common(1), Err(ResourceError::WeightExhausted));
    assert!(meter.is_exhausted());
    assert_eq!(meter.charge_common(0), Err(ResourceError::WeightExhausted));
    assert_eq!(
        meter.charge(ResourceDomain::Native, 0),
        Err(ResourceError::WeightExhausted)
    );
}

#[test]
fn exact_budget_succeeds_then_overrun_exhausts() {
    let mut meter = WeightMeter::new(unit_schedule(), 3, 1).unwrap();
    meter.charge(ResourceDomain::Native, 2).unwrap();
    assert_eq!(meter.consumed(), 3);
    assert_eq!(meter.remaining(), 0);
    assert!(!meter.is_exhausted());
    assert_eq!(meter.charge_common(0), Ok(()));
    assert!(!meter.is_exhausted());
    assert_eq!(meter.charge_common(1), Err(ResourceError::WeightExhausted));
    assert_eq!(meter.consumed(), 3);
    assert!(meter.is_exhausted());
}

#[test]
fn invalid_budgets_are_rejected() {
    assert!(matches!(
        WeightMeter::new(unit_schedule(), 0, 0),
        Err(ResourceError::InvalidBudget)
    ));
    assert!(matches!(
        WeightMeter::new(unit_schedule(), 4, 5),
        Err(ResourceError::InvalidBudget)
    ));
}

#[test]
fn mixed_domains_and_common_work_share_one_budget() {
    let native = WeightRatio::new(1, 1).unwrap();
    let evm = WeightRatio::new(2, 3).unwrap();
    let wasm = WeightRatio::new(3, 2).unwrap();
    let schedule = MeterScheduleV1::new(1, native, evm, wasm).unwrap();
    let mut meter = WeightMeter::new(schedule, 10, 1).unwrap();
    meter.charge(ResourceDomain::Native, 2).unwrap();
    meter.charge(ResourceDomain::Evm, 1).unwrap();
    meter.charge(ResourceDomain::Wasm, 2).unwrap();
    meter.charge_common(3).unwrap();
    assert_eq!(meter.consumed(), 10);
    assert_eq!(meter.remaining(), 0);
    assert_eq!(meter.max_weight(), 10);
}

#[test]
fn zero_charges_leave_live_meter_unchanged() {
    let mut meter = WeightMeter::new(unit_schedule(), 5, 2).unwrap();
    for domain in [
        ResourceDomain::Native,
        ResourceDomain::Evm,
        ResourceDomain::Wasm,
    ] {
        assert_eq!(meter.charge(domain, 0), Ok(()));
    }
    assert_eq!(meter.charge_common(0), Ok(()));
    assert_eq!(meter.consumed(), 2);
    assert_eq!(meter.remaining(), 3);
}

#[test]
fn native_counter_overflow_is_terminal() {
    let tiny = WeightRatio::new(1, u64::MAX).unwrap();
    let schedule = MeterScheduleV1::new(1, tiny, tiny, tiny).unwrap();
    let mut meter = WeightMeter::new(schedule, u64::MAX, 0).unwrap();
    meter.charge(ResourceDomain::Native, u64::MAX).unwrap();
    assert_eq!(meter.consumed(), 1);
    assert_eq!(
        meter.charge(ResourceDomain::Native, 1),
        Err(ResourceError::WeightExhausted)
    );
    assert_eq!(meter.consumed(), u64::MAX);
    assert!(meter.is_exhausted());
}

#[test]
fn cumulative_conversion_overflow_is_terminal() {
    let huge = WeightRatio::new(u64::MAX, 1).unwrap();
    let schedule = MeterScheduleV1::new(1, huge, huge, huge).unwrap();
    let mut meter = WeightMeter::new(schedule, u64::MAX, 0).unwrap();
    assert_eq!(
        meter.charge(ResourceDomain::Wasm, 2),
        Err(ResourceError::WeightExhausted)
    );
    assert_eq!(meter.consumed(), u64::MAX);
    assert!(meter.is_exhausted());
}

proptest! {
    #[test]
    fn split_and_combined_charges_are_equivalent(
        numerator in 1u64..=20,
        denominator in 1u64..=20,
        chunks in prop::collection::vec(0u16..=100, 1..20),
    ) {
        let ratio = WeightRatio::new(numerator, denominator).unwrap();
        let schedule = MeterScheduleV1::new(1, ratio, ratio, ratio).unwrap();
        let mut split = WeightMeter::new(schedule, u64::MAX, 0).unwrap();
        let mut total = 0u64;
        for chunk in chunks {
            let chunk = u64::from(chunk);
            total += chunk;
            split.charge(ResourceDomain::Evm, chunk).unwrap();
        }
        let mut combined = WeightMeter::new(schedule, u64::MAX, 0).unwrap();
        combined.charge(ResourceDomain::Evm, total).unwrap();
        prop_assert_eq!(split.consumed(), combined.consumed());
    }

    #[test]
    fn domain_reordering_preserves_consumption(
        native in 0u16..=100,
        evm in 0u16..=100,
        wasm in 0u16..=100,
        common in 0u16..=100,
    ) {
        let schedule = MeterScheduleV1::new(
            1,
            WeightRatio::new(1, 1).unwrap(),
            WeightRatio::new(2, 3).unwrap(),
            WeightRatio::new(3, 2).unwrap(),
        ).unwrap();
        let mut first = WeightMeter::new(schedule, u64::MAX, 0).unwrap();
        first.charge(ResourceDomain::Native, u64::from(native)).unwrap();
        first.charge(ResourceDomain::Evm, u64::from(evm)).unwrap();
        first.charge(ResourceDomain::Wasm, u64::from(wasm)).unwrap();
        first.charge_common(u64::from(common)).unwrap();

        let mut second = WeightMeter::new(schedule, u64::MAX, 0).unwrap();
        second.charge_common(u64::from(common)).unwrap();
        second.charge(ResourceDomain::Wasm, u64::from(wasm)).unwrap();
        second.charge(ResourceDomain::Native, u64::from(native)).unwrap();
        second.charge(ResourceDomain::Evm, u64::from(evm)).unwrap();
        prop_assert_eq!(first.consumed(), second.consumed());
    }
}
