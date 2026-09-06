use oregon_consensus::{
    ConsensusError, ConsensusParams, Target, validate_coinbase_with_execution_fees_v1,
};
use oregon_primitives::{Amount, Hash256, Transaction, TxInput, TxOutput, write_varint};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Vectors {
    version: u64,
    producer_cases: Vec<ProducerCase>,
}

#[derive(Debug, Deserialize)]
struct ProducerCase {
    name: String,
    height: u64,
    native_fees: u64,
    execution_fees: u64,
    outputs: Vec<Output>,
    expected: String,
}

#[derive(Debug, Deserialize)]
struct Output {
    value: u64,
    locking_program_hex: String,
}

fn vectors() -> Vectors {
    serde_json::from_str(include_str!(
        "../../../tests/vectors/fee-settlement-v1.json"
    ))
    .expect("valid committed Stage 3B vectors")
}

fn bytes(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("lower hex byte"))
        .collect()
}

fn params() -> ConsensusParams {
    ConsensusParams::new(
        Target::from_le_bytes([0xff; 32]).unwrap(),
        Target::from_le_bytes([0x7f; 32]).unwrap(),
        [0x42; 32],
    )
    .unwrap()
}

fn coinbase(height: u64, outputs: &[Output]) -> Transaction {
    let mut height_bytes = Vec::new();
    write_varint(height, &mut height_bytes);
    Transaction {
        version: 1,
        inputs: vec![TxInput {
            previous_txid: Hash256::from_bytes([0u8; 32]),
            previous_output_index: u32::MAX,
            sequence: u32::MAX,
            witness: vec![height_bytes],
        }],
        outputs: outputs
            .iter()
            .map(|output| TxOutput {
                value: Amount::from_base_units(output.value).unwrap(),
                locking_program: bytes(&output.locking_program_hex),
            })
            .collect(),
        lock_time: 0,
    }
}

#[test]
fn independent_producer_vectors_pin_execution_fee_boundaries() {
    let vectors = vectors();
    assert_eq!(vectors.version, 1);
    let params = params();

    for case in vectors.producer_cases {
        let result = validate_coinbase_with_execution_fees_v1(
            &coinbase(case.height, &case.outputs),
            case.height,
            Amount::from_base_units(case.native_fees).unwrap(),
            Amount::from_base_units(case.execution_fees).unwrap(),
            &params,
        );
        let expected = match case.expected.as_str() {
            "ok" => Ok(()),
            "invalid_coinbase" => Err(ConsensusError::InvalidCoinbase),
            "coinbase_overclaim" => Err(ConsensusError::CoinbaseOverClaim),
            other => panic!("unknown vector expectation {other} for {}", case.name),
        };
        assert_eq!(result, expected, "producer vector {}", case.name);
    }
}
