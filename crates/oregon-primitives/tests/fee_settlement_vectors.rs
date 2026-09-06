use std::str::FromStr;

use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::execution_reserve::{reserve_outpoint_txid, reserve_transition_id};
use oregon_primitives::fee_settlement::{ExecutionOutcome, FeeSettlementReceiptV1, FeeSourceKind};
use oregon_primitives::Hash256;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Vectors {
    version: u64,
    settlement_case: SettlementCase,
    reserve_cases: Vec<ReserveCase>,
}

#[derive(Debug, Deserialize)]
struct SettlementCase {
    txid: String,
    payer_kind: u8,
    payer_payload: String,
    source_kind: u8,
    escrow_id: String,
    outcome: u8,
    actual_weight: u64,
    receipt_hex: String,
    receipt_id: String,
}

#[derive(Debug, Deserialize)]
struct ReserveCase {
    canonical_transition_hex: String,
    transition_id: String,
    reserve_outpoint_txid: Option<String>,
    reserve_outpoint_index: Option<u32>,
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
fn independent_receipt_vector_pins_exact_bytes_identity_and_fields() {
    let vectors = vectors();
    assert_eq!(vectors.version, 1);
    let case = vectors.settlement_case;
    let literal = bytes(&case.receipt_hex);
    let receipt = FeeSettlementReceiptV1::decode(&literal).expect("literal receipt decodes");

    assert_eq!(receipt.encode().as_slice(), literal.as_slice());
    assert_eq!(receipt.receipt_id(), hash(&case.receipt_id));
    assert_eq!(receipt.txid(), hash(&case.txid));
    assert_eq!(receipt.escrow_id(), hash(&case.escrow_id));
    assert_eq!(receipt.source_kind(), FeeSourceKind::try_from(case.source_kind).unwrap());
    assert_eq!(receipt.outcome(), ExecutionOutcome::try_from(case.outcome).unwrap());
    assert_eq!(receipt.actual_weight(), case.actual_weight);

    let payload: [u8; 32] = bytes(&case.payer_payload).try_into().unwrap();
    let payer = ExecutionAddress::new(
        ExecutionAddressKind::try_from(case.payer_kind).unwrap(),
        payload,
    )
    .unwrap();
    assert_eq!(receipt.payer(), payer);
}

#[test]
fn independent_reserve_vectors_pin_transition_and_outpoint_domains() {
    for case in vectors().reserve_cases {
        let preimage = bytes(&case.canonical_transition_hex);
        let transition_id = reserve_transition_id(&preimage);
        assert_eq!(transition_id, hash(&case.transition_id));

        match (case.reserve_outpoint_txid, case.reserve_outpoint_index) {
            (Some(expected_txid), Some(0)) => {
                assert_eq!(reserve_outpoint_txid(transition_id), hash(&expected_txid));
            }
            (None, None) => {}
            other => panic!("noncanonical reserve outpoint vector: {other:?}"),
        }
    }
}
