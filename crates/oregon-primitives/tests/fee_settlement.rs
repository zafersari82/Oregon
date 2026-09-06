use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::fee_settlement::{
    ExecutionOutcome, FeeSettlementError, FeeSettlementReceiptV1, FeeSettlementReceiptV1Parts,
    FeeSourceKind,
};
use oregon_primitives::{Hash256, domain_hash};

const RECEIPT_DOMAIN: &[u8] = b"OREGON/FEE/RECEIPT/V1\0";
const RECEIPT_BYTES: usize = 181;
const SOURCE_KIND_OFFSET: usize = 99;
const OUTCOME_OFFSET: usize = 100;

fn payer() -> ExecutionAddress {
    ExecutionAddress::new(ExecutionAddressKind::Oregon, [0x33; 32]).unwrap()
}

fn canonical_parts() -> FeeSettlementReceiptV1Parts {
    FeeSettlementReceiptV1Parts {
        txid: Hash256::from_bytes([0x11; 32]),
        escrow_id: Hash256::from_bytes([0x22; 32]),
        payer: payer(),
        source_kind: FeeSourceKind::ExecutionBalance,
        outcome: ExecutionOutcome::Reverted,
        base_fee_per_weight: 10,
        max_fee_per_weight: 20,
        max_priority_fee_per_weight: 5,
        max_weight: 100,
        actual_weight: 40,
        effective_price: 15,
        base_component: 400,
        priority_component: 200,
        charged: 600,
        refund: 1_400,
    }
}

#[test]
fn canonical_receipt_round_trips_and_hashes_exactly() {
    let receipt = FeeSettlementReceiptV1::new(canonical_parts()).unwrap();
    let bytes = receipt.encode();

    assert_eq!(bytes.len(), RECEIPT_BYTES);
    assert_eq!(FeeSettlementReceiptV1::decode(&bytes).unwrap(), receipt);
    assert_eq!(receipt.receipt_id(), domain_hash(RECEIPT_DOMAIN, &bytes));
}

#[test]
fn receipt_rejects_zero_actual_weight() {
    let mut parts = canonical_parts();
    parts.actual_weight = 0;
    parts.base_component = 0;
    parts.priority_component = 0;
    parts.charged = 0;
    parts.refund = 2_000;

    assert_eq!(
        FeeSettlementReceiptV1::new(parts),
        Err(FeeSettlementError::InvalidActualWeight)
    );
}

#[test]
fn receipt_rejects_actual_weight_above_max_weight() {
    let mut parts = canonical_parts();
    parts.actual_weight = 101;

    assert_eq!(
        FeeSettlementReceiptV1::new(parts),
        Err(FeeSettlementError::InvalidActualWeight)
    );
}

#[test]
fn receipt_rejects_max_fee_below_base_fee() {
    let mut parts = canonical_parts();
    parts.base_fee_per_weight = 21;

    assert_eq!(
        FeeSettlementReceiptV1::new(parts),
        Err(FeeSettlementError::MaxFeeBelowBaseFee)
    );
}

#[test]
fn receipt_rejects_inconsistent_effective_price() {
    let mut parts = canonical_parts();
    parts.effective_price = 14;

    assert_eq!(
        FeeSettlementReceiptV1::new(parts),
        Err(FeeSettlementError::InconsistentArithmetic)
    );
}

#[test]
fn receipt_rejects_inconsistent_refund() {
    let mut parts = canonical_parts();
    parts.refund = 1_399;

    assert_eq!(
        FeeSettlementReceiptV1::new(parts),
        Err(FeeSettlementError::InconsistentArithmetic)
    );
}

#[test]
fn decode_rejects_unknown_fee_source_kind() {
    let mut bytes = FeeSettlementReceiptV1::new(canonical_parts())
        .unwrap()
        .encode();
    bytes[SOURCE_KIND_OFFSET] = 0xff;

    assert_eq!(
        FeeSettlementReceiptV1::decode(&bytes),
        Err(FeeSettlementError::UnknownFeeSourceKind(0xff))
    );
}

#[test]
fn decode_rejects_unknown_execution_outcome() {
    let mut bytes = FeeSettlementReceiptV1::new(canonical_parts())
        .unwrap()
        .encode();
    bytes[OUTCOME_OFFSET] = 0xff;

    assert_eq!(
        FeeSettlementReceiptV1::decode(&bytes),
        Err(FeeSettlementError::UnknownExecutionOutcome(0xff))
    );
}

#[test]
fn decode_rejects_wrong_fixed_width() {
    let bytes = vec![0u8; RECEIPT_BYTES - 1];
    assert_eq!(
        FeeSettlementReceiptV1::decode(&bytes),
        Err(FeeSettlementError::InvalidReceiptLength(RECEIPT_BYTES - 1))
    );
}
