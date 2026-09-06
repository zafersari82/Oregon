use oregon_execution::{EscrowBookV1, FeeError, FeeTermsV1, FundingCapabilityV1};
use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::fee_settlement::{ExecutionOutcome, FeeSourceKind};
use oregon_primitives::{Hash256, MAX_SUPPLY_BASE_UNITS, domain_hash};

fn payer() -> ExecutionAddress {
    ExecutionAddress::new(ExecutionAddressKind::Oregon, [0x55; 32]).unwrap()
}

fn execution_capability_with_auth(
    available: u64,
    sequence: u64,
    authorization_tag: u8,
) -> FundingCapabilityV1 {
    FundingCapabilityV1::new(
        FeeSourceKind::ExecutionBalance,
        payer(),
        Hash256::from_bytes([0x66; 32]),
        available,
        sequence,
        Hash256::from_bytes([authorization_tag; 32]),
    )
    .unwrap()
}

fn execution_capability(available: u64, sequence: u64) -> FundingCapabilityV1 {
    execution_capability_with_auth(available, sequence, 0x77)
}

fn terms() -> FeeTermsV1 {
    FeeTermsV1::new(10, 20, 5, 100).unwrap()
}

#[test]
fn max_fee_below_base_fee_is_rejected() {
    assert_eq!(
        FeeTermsV1::new(10, 9, 1, 100),
        Err(FeeError::MaxFeeBelowBaseFee)
    );
}

#[test]
fn zero_weight_and_priority_above_max_fee_are_rejected() {
    assert_eq!(FeeTermsV1::new(10, 20, 5, 0), Err(FeeError::ZeroMaxWeight));
    assert_eq!(
        FeeTermsV1::new(10, 20, 21, 100),
        Err(FeeError::PriorityFeeExceedsMaxFee)
    );
}

#[test]
fn max_escrow_uses_wide_arithmetic_and_supply_bound() {
    assert_eq!(
        FeeTermsV1::new(1, 2, 0, u64::MAX),
        Err(FeeError::ArithmeticOverflow)
    );
    assert_eq!(
        FeeTermsV1::new(1, 1, 0, MAX_SUPPLY_BASE_UNITS + 1),
        Err(FeeError::AmountExceedsMaximumSupply)
    );
}

#[test]
fn funding_capability_id_binds_every_canonical_field() {
    let capability = execution_capability(2_000, 7);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.push(FeeSourceKind::ExecutionBalance as u8);
    bytes.extend_from_slice(&payer().to_bytes());
    bytes.extend_from_slice(Hash256::from_bytes([0x66; 32]).as_bytes());
    bytes.extend_from_slice(&2_000u64.to_le_bytes());
    bytes.extend_from_slice(&7u64.to_le_bytes());
    bytes.extend_from_slice(Hash256::from_bytes([0x77; 32]).as_bytes());

    assert_eq!(
        capability.capability_id(),
        domain_hash(b"OREGON/FEE/CAPABILITY/V1\0", &bytes)
    );
}

#[test]
fn escrow_ticket_id_binds_transaction_capability_and_terms() {
    let mut book = EscrowBookV1::new(8).unwrap();
    let capability = execution_capability(2_000, 7);
    let capability_id = capability.capability_id();
    let txid = Hash256::from_bytes([0x44; 32]);
    let ticket = book.open(txid, capability, terms()).unwrap();

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(txid.as_bytes());
    bytes.extend_from_slice(&payer().to_bytes());
    bytes.push(FeeSourceKind::ExecutionBalance as u8);
    bytes.extend_from_slice(capability_id.as_bytes());
    for value in [10u64, 20, 5, 100, 2_000] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    assert_eq!(
        ticket.escrow_id(),
        domain_hash(b"OREGON/FEE/ESCROW/V1\0", &bytes)
    );
}

#[test]
fn reverted_execution_still_charges_consumed_weight_once() {
    let mut book = EscrowBookV1::new(8).unwrap();
    let capability = execution_capability(2_000, 7);
    let ticket = book
        .open(Hash256::from_bytes([0x44; 32]), capability, terms())
        .unwrap();

    let settled = book
        .settle(ticket.escrow_id(), ExecutionOutcome::Reverted, 40)
        .unwrap();
    assert_eq!(settled.charged(), 600);
    assert_eq!(settled.refund(), 1_400);
    assert_eq!(settled.receipt().outcome(), ExecutionOutcome::Reverted);
    assert_eq!(
        book.settle(ticket.escrow_id(), ExecutionOutcome::Reverted, 40),
        Err(FeeError::EscrowAlreadySettled)
    );
}

#[test]
fn actual_weight_boundary_is_atomic() {
    let mut book = EscrowBookV1::new(8).unwrap();
    let ticket = book
        .open(
            Hash256::from_bytes([0x45; 32]),
            execution_capability(2_000, 8),
            terms(),
        )
        .unwrap();

    assert_eq!(
        book.settle(ticket.escrow_id(), ExecutionOutcome::Committed, 101),
        Err(FeeError::ActualWeightOutOfRange)
    );

    let exact = book
        .settle(ticket.escrow_id(), ExecutionOutcome::Committed, 100)
        .unwrap();
    assert_eq!(exact.charged(), 1_500);
    assert_eq!(exact.refund(), 500);
}

#[test]
fn insufficient_funding_rejects_without_consuming_capability() {
    let mut book = EscrowBookV1::new(8).unwrap();
    let capability = execution_capability(1_999, 9);
    assert_eq!(
        book.open(Hash256::from_bytes([0x46; 32]), capability, terms(),),
        Err(FeeError::InsufficientFunding)
    );

    let lower_terms = FeeTermsV1::new(10, 19, 5, 100).unwrap();
    assert!(
        book.open(Hash256::from_bytes([0x46; 32]), capability, lower_terms,)
            .is_ok()
    );
}

#[test]
fn duplicate_capability_and_stale_source_sequence_fail_closed() {
    let mut book = EscrowBookV1::new(8).unwrap();
    let capability = execution_capability(2_000, 10);
    book.open(Hash256::from_bytes([0x47; 32]), capability, terms())
        .unwrap();

    assert_eq!(
        book.open(Hash256::from_bytes([0x48; 32]), capability, terms(),),
        Err(FeeError::CapabilityAlreadyConsumed)
    );

    let same_source_new_authorization = execution_capability_with_auth(2_000, 10, 0x78);
    assert_eq!(
        book.open(
            Hash256::from_bytes([0x49; 32]),
            same_source_new_authorization,
            terms(),
        ),
        Err(FeeError::StaleFundingSource)
    );
}

#[test]
fn unknown_escrow_and_zero_book_capacity_are_rejected() {
    assert!(matches!(
        EscrowBookV1::new(0),
        Err(FeeError::ZeroEscrowCapacity)
    ));

    let mut book = EscrowBookV1::new(1).unwrap();
    assert_eq!(
        book.settle(
            Hash256::from_bytes([0x99; 32]),
            ExecutionOutcome::Committed,
            1,
        ),
        Err(FeeError::UnknownEscrow)
    );
}

#[test]
fn open_escrow_capacity_is_enforced_before_mutation() {
    let mut book = EscrowBookV1::new(1).unwrap();
    let first = book
        .open(
            Hash256::from_bytes([0x50; 32]),
            execution_capability(2_000, 11),
            terms(),
        )
        .unwrap();

    assert_eq!(
        book.open(
            Hash256::from_bytes([0x51; 32]),
            execution_capability(2_000, 12),
            terms(),
        ),
        Err(FeeError::EscrowCapacityExceeded)
    );

    book.settle(first.escrow_id(), ExecutionOutcome::Committed, 1)
        .unwrap();
    assert!(
        book.open(
            Hash256::from_bytes([0x51; 32]),
            execution_capability(2_000, 12),
            terms(),
        )
        .is_ok()
    );
}

#[test]
fn resource_exhaustion_is_a_fee_paying_reverted_outcome() {
    let mut book = EscrowBookV1::new(8).unwrap();
    let ticket = book
        .open(
            Hash256::from_bytes([0x52; 32]),
            execution_capability(2_000, 13),
            terms(),
        )
        .unwrap();

    let settled = book
        .settle(ticket.escrow_id(), ExecutionOutcome::ResourceExhausted, 100)
        .unwrap();
    assert_eq!(settled.charged(), 1_500);
    assert_eq!(settled.refund(), 500);
    assert_eq!(
        settled.receipt().outcome(),
        ExecutionOutcome::ResourceExhausted
    );
}
