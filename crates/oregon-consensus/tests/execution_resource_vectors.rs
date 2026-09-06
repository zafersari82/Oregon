use oregon_consensus::execution_resources::{FeeParametersV1, next_base_fee};
use serde::Deserialize;

#[derive(Deserialize)]
struct FeeCase {
    parameters: [u64; 6],
    parent_base_fee: u64,
    parent_weight: u64,
    expected: u64,
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
struct Vectors {
    fees: Vec<FeeCase>,
    sequences: Vec<Sequence>,
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
}
