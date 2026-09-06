use oregon_execution::{MeterScheduleV1, ResourceDomain, ResourceError, WeightMeter, WeightRatio};
use serde::Deserialize;

#[derive(Deserialize)]
struct Conversion {
    numerator: u64,
    denominator: u64,
    units: u64,
    expected: Option<u64>,
}

#[derive(Deserialize)]
struct Event {
    domain: String,
    units: u64,
    consumed: u64,
    exhausted: bool,
    failed: bool,
}

#[derive(Deserialize)]
struct MeterCase {
    name: String,
    ratios: [[u64; 2]; 3],
    max_weight: u64,
    intrinsic_weight: u64,
    events: Vec<Event>,
}

#[derive(Deserialize)]
struct Vectors {
    conversions: Vec<Conversion>,
    meters: Vec<MeterCase>,
}

fn vectors() -> Vectors {
    serde_json::from_str(include_str!(
        "../../../tests/vectors/execution-resources-v1.json"
    ))
    .unwrap()
}

#[test]
fn independent_conversion_vectors_cover_wide_products() {
    let cases = vectors().conversions;
    assert!(!cases.is_empty());
    for c in cases {
        let result = WeightRatio::new(c.numerator, c.denominator)
            .unwrap()
            .normalized_weight(c.units);
        assert_eq!(
            result,
            c.expected.ok_or(ResourceError::WeightOverflow),
            "units {}, ratio {}/{}",
            c.units,
            c.numerator,
            c.denominator
        );
    }
}

#[test]
fn independent_meter_vectors_bind_each_transition() {
    let cases = vectors().meters;
    assert!(!cases.is_empty());
    for c in cases {
        let ratios = c.ratios.map(|r| WeightRatio::new(r[0], r[1]).unwrap());
        let schedule = MeterScheduleV1::new(1, ratios[0], ratios[1], ratios[2]).unwrap();
        let mut meter = WeightMeter::new(schedule, c.max_weight, c.intrinsic_weight).unwrap();
        assert!(!c.events.is_empty());
        for event in c.events {
            let result = match event.domain.as_str() {
                "native" => meter.charge(ResourceDomain::Native, event.units),
                "evm" => meter.charge(ResourceDomain::Evm, event.units),
                "wasm" => meter.charge(ResourceDomain::Wasm, event.units),
                "common" => meter.charge_common(event.units),
                other => panic!("unknown vector domain {other}"),
            };
            let expected = if event.failed {
                Err(ResourceError::WeightExhausted)
            } else {
                Ok(())
            };
            assert_eq!(result, expected, "case {}", c.name);
            assert_eq!(meter.consumed(), event.consumed, "case {}", c.name);
            assert_eq!(
                meter.remaining(),
                c.max_weight - event.consumed,
                "case {}",
                c.name
            );
            assert_eq!(meter.is_exhausted(), event.exhausted, "case {}", c.name);
        }
    }
}
