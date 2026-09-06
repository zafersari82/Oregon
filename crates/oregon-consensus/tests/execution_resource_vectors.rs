use oregon_consensus::execution_resources::{
    BlockWeightBudget, FeeParametersV1, ResourceFeeError, next_base_fee,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct FeeCase {
    parameters: [u64; 6],
    parent_base_fee: u64,
    parent_weight: u64,
    expected: u64,
}

#[derive(Deserialize)]
struct InvalidFeeCase {
    parameters: [u64; 6],
    parent_base_fee: u64,
    parent_weight: u64,
}

#[derive(Deserialize)]
struct Sequence {
    name: String,
    parameters: [u64; 6],
    initial_fee: u64,
    parent_weights: Vec<u64>,
    expected_fees: Vec<u64>,
}

#[derive(Deserialize)]
struct BlockBudgetCase {
    parameters: [u64; 6],
    attempts: Vec<u64>,
    expected: Vec<String>,
}

#[derive(Deserialize)]
struct Vectors {
    fees: Vec<FeeCase>,
    sequences: Vec<Sequence>,
    invalid_fee_cases: Vec<InvalidFeeCase>,
    block_budget_cases: Vec<BlockBudgetCase>,
    invalid_schedule_versions: Vec<u64>,
}

fn parameters(v: [u64; 6]) -> FeeParametersV1 {
    FeeParametersV1::new(v[0].try_into().unwrap(), v[1], v[2], v[3], v[4], v[5]).unwrap()
}

#[test]
fn independent_fee_vectors_and_producer_sequences() {
    let vectors: Vectors = serde_json::from_str(include_str!(
        "../../../tests/vectors/execution-resources-v1.json"
    ))
    .unwrap();
    assert!(!vectors.fees.is_empty());
    assert!(!vectors.sequences.is_empty());
    for case in vectors.fees {
        assert_eq!(
            next_base_fee(
                &parameters(case.parameters),
                case.parent_base_fee,
                case.parent_weight
            ),
            Ok(case.expected),
            "parent fee {}, used {}",
            case.parent_base_fee,
            case.parent_weight
        );
    }
    for sequence in vectors.sequences {
        assert!(!sequence.parent_weights.is_empty());
        assert_eq!(sequence.parent_weights.len(), sequence.expected_fees.len());
        let p = parameters(sequence.parameters);
        let mut previous = sequence.initial_fee;
        for (used, expected) in sequence
            .parent_weights
            .into_iter()
            .zip(sequence.expected_fees)
        {
            previous = next_base_fee(&p, previous, used).unwrap();
            assert_eq!(previous, expected, "sequence {}", sequence.name);
        }
    }
    for case in vectors.invalid_fee_cases {
        assert_eq!(
            next_base_fee(
                &parameters(case.parameters),
                case.parent_base_fee,
                case.parent_weight
            ),
            Err(ResourceFeeError::ParentWeightExceeded)
        );
    }
    assert_eq!(vectors.invalid_schedule_versions, vec![0, 2]);
    for version in vectors.invalid_schedule_versions {
        assert_eq!(
            FeeParametersV1::new(version as u16, 100, 100, 8, 1, 1_000),
            Err(ResourceFeeError::UnsupportedVersion(version as u16))
        );
    }
    assert!(!vectors.block_budget_cases.is_empty());
    for case in vectors.block_budget_cases {
        assert_eq!(case.attempts.len(), case.expected.len());
        let mut budget = BlockWeightBudget::new(&parameters(case.parameters));
        let mut expected_consumed = 0;
        for (attempt, expected) in case.attempts.into_iter().zip(case.expected) {
            let result = budget.include(attempt);
            match expected.as_str() {
                "ok" => {
                    assert_eq!(result, Ok(()));
                    expected_consumed += attempt;
                }
                "invalid_transaction" => {
                    assert_eq!(result, Err(ResourceFeeError::InvalidTransactionWeight));
                }
                "block_exceeded" => {
                    assert_eq!(result, Err(ResourceFeeError::BlockWeightExceeded));
                }
                other => panic!("unknown block-budget vector outcome: {other}"),
            }
            assert_eq!(budget.consumed(), expected_consumed);
        }
    }
}
