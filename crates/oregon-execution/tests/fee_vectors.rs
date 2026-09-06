use std::str::FromStr;

use oregon_execution::{EscrowBookV1, FeeTermsV1, FundingCapabilityV1};
use oregon_primitives::Hash256;
use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::fee_settlement::{ExecutionOutcome, FeeSourceKind};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Vectors {
    version: u64,
    fee_cases: Vec<FeeCase>,
    settlement_case: SettlementCase,
}

#[derive(Debug, Deserialize)]
struct FeeCase {
    base_fee_per_weight: u64,
    max_fee_per_weight: u64,
    max_priority_fee_per_weight: u64,
    max_weight: u64,
    actual_weight: u64,
    max_escrow: u64,
    effective_price: u64,
    charged: u64,
    base_component: u64,
    priority_component: u64,
    refund: u64,
}

#[derive(Debug, Deserialize)]
struct SettlementCase {
    txid: String,
    payer_kind: u8,
    payer_payload: String,
    source_kind: u8,
    source_commitment: String,
    available_amount: u64,
    source_sequence: u64,
    authorization_commitment: String,
    capability_id: String,
    terms: Terms,
    escrow_id: String,
    outcome: u8,
    actual_weight: u64,
    receipt_hex: String,
    receipt_id: String,
}

#[derive(Debug, Deserialize)]
struct Terms {
    base_fee_per_weight: u64,
    max_fee_per_weight: u64,
    max_priority_fee_per_weight: u64,
    max_weight: u64,
    max_escrow: u64,
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

fn payer(kind: u8, payload: &str) -> ExecutionAddress {
    let payload: [u8; 32] = bytes(payload).try_into().unwrap();
    ExecutionAddress::new(ExecutionAddressKind::try_from(kind).unwrap(), payload).unwrap()
}

#[test]
fn independent_fee_vectors_bind_checked_arithmetic_and_settlement() {
    let vectors = vectors();
    assert_eq!(vectors.version, 1);

    for (index, case) in vectors.fee_cases.into_iter().enumerate() {
        let terms = FeeTermsV1::new(
            case.base_fee_per_weight,
            case.max_fee_per_weight,
            case.max_priority_fee_per_weight,
            case.max_weight,
        )
        .unwrap();
        assert_eq!(terms.max_escrow(), case.max_escrow);

        let capability = FundingCapabilityV1::new(
            FeeSourceKind::ExecutionBalance,
            ExecutionAddress::new(ExecutionAddressKind::Oregon, [0x35; 32]).unwrap(),
            Hash256::from_bytes([0x46; 32]),
            case.max_escrow,
            index as u64,
            Hash256::from_bytes([0x57; 32]),
        )
        .unwrap();
        let mut book = EscrowBookV1::new(1).unwrap();
        let ticket = book
            .open(Hash256::from_bytes([index as u8; 32]), capability, terms)
            .unwrap();
        let settled = book
            .settle(
                ticket.escrow_id(),
                ExecutionOutcome::Committed,
                case.actual_weight,
            )
            .unwrap();
        let parts = settled.receipt().parts();

        assert_eq!(parts.effective_price, case.effective_price);
        assert_eq!(parts.base_component, case.base_component);
        assert_eq!(parts.priority_component, case.priority_component);
        assert_eq!(settled.charged(), case.charged);
        assert_eq!(settled.refund(), case.refund);
    }
}

#[test]
fn independent_settlement_vector_pins_capability_escrow_and_receipt_ids() {
    let case = vectors().settlement_case;
    let capability = FundingCapabilityV1::new(
        FeeSourceKind::try_from(case.source_kind).unwrap(),
        payer(case.payer_kind, &case.payer_payload),
        hash(&case.source_commitment),
        case.available_amount,
        case.source_sequence,
        hash(&case.authorization_commitment),
    )
    .unwrap();
    assert_eq!(capability.capability_id(), hash(&case.capability_id));

    let terms = FeeTermsV1::new(
        case.terms.base_fee_per_weight,
        case.terms.max_fee_per_weight,
        case.terms.max_priority_fee_per_weight,
        case.terms.max_weight,
    )
    .unwrap();
    assert_eq!(terms.max_escrow(), case.terms.max_escrow);

    let mut book = EscrowBookV1::new(1).unwrap();
    let ticket = book.open(hash(&case.txid), capability, terms).unwrap();
    assert_eq!(ticket.escrow_id(), hash(&case.escrow_id));

    let settled = book
        .settle(
            ticket.escrow_id(),
            ExecutionOutcome::try_from(case.outcome).unwrap(),
            case.actual_weight,
        )
        .unwrap();
    assert_eq!(
        settled.receipt().encode().as_slice(),
        bytes(&case.receipt_hex)
    );
    assert_eq!(settled.receipt().receipt_id(), hash(&case.receipt_id));
}
