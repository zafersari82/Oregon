use std::str::FromStr;

use oregon_primitives::{Hash256, OutPoint};
use oregon_utxo::{ReservePoolSnapshotV1, ReserveTransitionV1, ReserveTransitionV1Parts};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Vectors {
    version: u64,
    reserve_cases: Vec<ReserveCase>,
}

#[derive(Debug, Deserialize)]
struct ReserveCase {
    name: String,
    chain_id: u64,
    height: u64,
    parent_block_hash: String,
    previous: Option<Previous>,
    native_deposit_total: u64,
    execution_withdrawal_total: u64,
    execution_fee_total: u64,
    new_execution_balance_total: u64,
    producer_coinbase_txid: String,
    canonical_transition_hex: String,
    transition_id: String,
    reserve_outpoint_txid: Option<String>,
    reserve_outpoint_index: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct Previous {
    txid: String,
    index: u32,
    amount: u64,
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

fn hash(hex: &str) -> Hash256 {
    Hash256::from_str(hex).expect("canonical hash")
}

#[test]
fn independent_reserve_vectors_pin_canonical_transition_and_singleton_outpoint() {
    let vectors = vectors();
    assert_eq!(vectors.version, 1);

    for case in vectors.reserve_cases {
        let previous = case.previous.map(|previous| ReservePoolSnapshotV1 {
            outpoint: OutPoint {
                txid: hash(&previous.txid),
                index: previous.index,
            },
            amount: previous.amount,
        });
        let transition = ReserveTransitionV1::new(ReserveTransitionV1Parts {
            chain_id: case.chain_id,
            height: case.height,
            parent_block_hash: hash(&case.parent_block_hash),
            previous,
            native_deposit_total: case.native_deposit_total,
            execution_withdrawal_total: case.execution_withdrawal_total,
            execution_fee_total: case.execution_fee_total,
            new_execution_balance_total: case.new_execution_balance_total,
            producer_coinbase_txid: hash(&case.producer_coinbase_txid),
        })
        .unwrap();

        assert_eq!(
            transition.canonical_transition_bytes(),
            bytes(&case.canonical_transition_hex),
            "reserve vector {} preimage",
            case.name
        );
        assert_eq!(
            transition.transition_id(),
            hash(&case.transition_id),
            "reserve vector {} id",
            case.name
        );

        match (case.reserve_outpoint_txid, case.reserve_outpoint_index) {
            (Some(txid), Some(index)) => assert_eq!(
                transition.new_reserve_outpoint(),
                Some(OutPoint {
                    txid: hash(&txid),
                    index,
                }),
                "reserve vector {} outpoint",
                case.name
            ),
            (None, None) => assert_eq!(
                transition.new_reserve_outpoint(),
                None,
                "reserve vector {} zero result",
                case.name
            ),
            other => panic!("noncanonical reserve outpoint vector {other:?}"),
        }
    }
}
