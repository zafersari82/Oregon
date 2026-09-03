use std::{fs, path::PathBuf};

use num_bigint::BigUint;
use oregon_consensus::{
    ConsensusParams, SCHEDULED_MINING_ISSUANCE_BASE_UNITS, SCHEDULED_TOTAL_WITH_FOUNDER_BASE_UNITS,
    Target, block_subsidy, block_work, required_target,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Fixture {
    target: TargetVector,
    emission: EmissionVector,
    asert: Vec<AsertVector>,
    work: WorkVector,
}

#[derive(Debug, Deserialize)]
struct TargetVector {
    little_endian_hex: String,
    integer_decimal: String,
}

#[derive(Debug, Deserialize)]
struct EmissionVector {
    height_1: u64,
    height_200000: u64,
    height_200001: u64,
    era_27_first: u64,
    era_28_first: u64,
    scheduled_mining: u64,
    scheduled_with_founder: u64,
}

#[derive(Debug, Deserialize)]
struct AsertVector {
    name: String,
    height: u64,
    parent_delta: i64,
    expected_target: String,
}

#[derive(Debug, Deserialize)]
struct WorkVector {
    target_max_work: String,
    target_one_work_hex: String,
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/vectors/consensus-m1-v1.json")
}

fn read_fixture() -> Fixture {
    let path = fixture_path();
    let bytes = fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "missing Oregon M1 golden fixture {}: {error}",
            path.display()
        )
    });
    serde_json::from_slice(&bytes).expect("Oregon M1 golden fixture must be valid JSON")
}

fn target(value: u64) -> Target {
    Target::from_biguint(&BigUint::from(value)).unwrap()
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

#[test]
fn consensus_m1_v1_vectors_match_public_consensus_apis() {
    let fixture = read_fixture();

    let mut target_bytes = [0u8; 32];
    target_bytes[0] = 0x34;
    target_bytes[1] = 0x12;
    let encoded_target = Target::from_le_bytes(target_bytes).unwrap();
    assert_eq!(
        lower_hex(&encoded_target.to_le_bytes()),
        fixture.target.little_endian_hex
    );
    assert_eq!(
        encoded_target.to_biguint().to_str_radix(10),
        fixture.target.integer_decimal
    );

    assert_eq!(
        block_subsidy(1).unwrap().base_units(),
        fixture.emission.height_1
    );
    assert_eq!(
        block_subsidy(200_000).unwrap().base_units(),
        fixture.emission.height_200000
    );
    assert_eq!(
        block_subsidy(200_001).unwrap().base_units(),
        fixture.emission.height_200001
    );
    assert_eq!(
        block_subsidy(27 * 200_000 + 1).unwrap().base_units(),
        fixture.emission.era_27_first
    );
    assert_eq!(
        block_subsidy(28 * 200_000 + 1).unwrap().base_units(),
        fixture.emission.era_28_first
    );
    assert_eq!(
        SCHEDULED_MINING_ISSUANCE_BASE_UNITS,
        fixture.emission.scheduled_mining
    );
    assert_eq!(
        SCHEDULED_TOTAL_WITH_FOUNDER_BASE_UNITS,
        fixture.emission.scheduled_with_founder
    );

    let genesis_timestamp = 1_800_000_000u64;
    let params = ConsensusParams::new(target(10_000_000), target(1_000_000), [0x42; 32]).unwrap();
    for vector in &fixture.asert {
        let parent_timestamp = if vector.parent_delta >= 0 {
            genesis_timestamp + vector.parent_delta as u64
        } else {
            genesis_timestamp - vector.parent_delta.unsigned_abs()
        };
        let actual = required_target(vector.height, parent_timestamp, genesis_timestamp, &params)
            .unwrap()
            .to_biguint()
            .to_str_radix(10);
        assert_eq!(
            actual, vector.expected_target,
            "ASERT vector {}",
            vector.name
        );
    }

    let max_target = Target::from_le_bytes([0xff; 32]).unwrap();
    assert_eq!(
        block_work(max_target).to_biguint().to_str_radix(10),
        fixture.work.target_max_work
    );
    let one = Target::from_biguint(&BigUint::from(1u8)).unwrap();
    assert_eq!(
        format!("{:064x}", block_work(one).to_biguint()),
        fixture.work.target_one_work_hex
    );
}
