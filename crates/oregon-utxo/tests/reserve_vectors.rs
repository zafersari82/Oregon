use std::str::FromStr;

use oregon_primitives::{Hash256, OutPoint};
use oregon_utxo::{ReservePoolSnapshotV1, ReserveTransitionV1, ReserveTransitionV1Parts};

const VECTORS: &str = include_str!("../../../tests/vectors/fee-settlement-v1.json");

fn case_slice<'a>(name: &str, next: Option<&str>) -> &'a str {
    let marker = format!("\"name\": \"{name}\"");
    assert_eq!(VECTORS.matches(&marker).count(), 1, "unique reserve vector {name}");
    let start = VECTORS.find(&marker).unwrap();
    let tail = &VECTORS[start..];
    let end = match next {
        Some(next_name) => {
            let next_marker = format!("\"name\": \"{next_name}\"");
            tail.find(&next_marker).expect("next reserve vector")
        }
        None => tail.find("\"producer_cases\"").expect("producer vector section"),
    };
    &tail[..end]
}

fn field_tail<'a>(case: &'a str, key: &str) -> &'a str {
    let marker = format!("\"{key}\": ");
    assert_eq!(case.matches(&marker).count(), 1, "unique field {key}");
    &case[case.find(&marker).unwrap() + marker.len()..]
}

fn string_field(case: &str, key: &str) -> Option<String> {
    let tail = field_tail(case, key);
    if tail.starts_with("null") {
        return None;
    }
    let tail = tail.strip_prefix('"').expect("quoted vector string");
    let end = tail.find('"').expect("closing vector quote");
    Some(tail[..end].to_owned())
}

fn u64_field(case: &str, key: &str) -> Option<u64> {
    let tail = field_tail(case, key);
    if tail.starts_with("null") {
        return None;
    }
    let end = tail
        .find(|character: char| !character.is_ascii_digit())
        .expect("numeric vector delimiter");
    Some(tail[..end].parse().expect("decimal vector integer"))
}

fn bytes(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("lower hex byte"))
        .collect()
}

fn hash(hex: &str) -> Hash256 {
    Hash256::from_str(hex).expect("canonical hash")
}

#[test]
fn independent_reserve_vectors_pin_canonical_transition_and_singleton_outpoint() {
    assert!(VECTORS.contains("\"version\": 1"));

    for (name, next) in [
        ("zero_deposit", Some("rebalance")),
        ("rebalance", Some("zero_result")),
        ("zero_result", None),
    ] {
        let case = case_slice(name, next);
        let previous = if case.contains("\"previous\": null") {
            None
        } else {
            Some(ReservePoolSnapshotV1 {
                outpoint: OutPoint {
                    txid: hash(&string_field(case, "txid").expect("previous txid")),
                    index: u64_field(case, "index").expect("previous index") as u32,
                },
                amount: u64_field(case, "amount").expect("previous amount"),
            })
        };

        let transition = ReserveTransitionV1::new(ReserveTransitionV1Parts {
            chain_id: u64_field(case, "chain_id").unwrap(),
            height: u64_field(case, "height").unwrap(),
            parent_block_hash: hash(&string_field(case, "parent_block_hash").unwrap()),
            previous,
            native_deposit_total: u64_field(case, "native_deposit_total").unwrap(),
            execution_withdrawal_total: u64_field(case, "execution_withdrawal_total").unwrap(),
            execution_fee_total: u64_field(case, "execution_fee_total").unwrap(),
            new_execution_balance_total: u64_field(case, "new_execution_balance_total").unwrap(),
            producer_coinbase_txid: hash(&string_field(case, "producer_coinbase_txid").unwrap()),
        })
        .unwrap();

        assert_eq!(
            transition.canonical_transition_bytes(),
            bytes(&string_field(case, "canonical_transition_hex").unwrap()),
            "reserve vector {name} preimage"
        );
        assert_eq!(
            transition.transition_id(),
            hash(&string_field(case, "transition_id").unwrap()),
            "reserve vector {name} id"
        );

        match (
            string_field(case, "reserve_outpoint_txid"),
            u64_field(case, "reserve_outpoint_index"),
        ) {
            (Some(txid), Some(index)) => assert_eq!(
                transition.new_reserve_outpoint(),
                Some(OutPoint {
                    txid: hash(&txid),
                    index: u32::try_from(index).unwrap(),
                }),
                "reserve vector {name} outpoint"
            ),
            (None, None) => assert_eq!(
                transition.new_reserve_outpoint(),
                None,
                "reserve vector {name} zero result"
            ),
            other => panic!("noncanonical reserve outpoint vector {other:?}"),
        }
    }
}
