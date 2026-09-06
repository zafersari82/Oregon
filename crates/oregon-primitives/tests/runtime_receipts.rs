use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::execution_effect::{StateEffectDescriptorV1, state_effect_root};
use oregon_primitives::execution_envelope::ExecutionDomain;
use oregon_primitives::execution_event::{
    ExecutionEventV1, MAX_EXECUTION_EVENT_DATA_BYTES_V1, MAX_EXECUTION_EVENT_TOPICS_V1,
    events_root,
};
use oregon_primitives::execution_receipt::{
    ExecutionReceiptError, ExecutionReceiptOutcomeV1, ExecutionReceiptV1,
    ExecutionReceiptV1Parts, EXECUTION_RECEIPT_BYTES_V1, empty_outbox_effect_root,
    return_data_hash,
};
use oregon_primitives::fee_settlement::{
    ExecutionOutcome, FeeSettlementReceiptV1, FeeSettlementReceiptV1Parts, FeeSourceKind,
};
use oregon_primitives::state_commitment::{CommitmentDomainId, CommitmentSchemeId};
use oregon_primitives::Hash256;

fn hash(fill: u8) -> Hash256 {
    Hash256::from_slice(&[fill; 32]).unwrap()
}

fn wasm(fill: u8) -> ExecutionAddress {
    ExecutionAddress::new(ExecutionAddressKind::Wasm, [fill; 32]).unwrap()
}

fn fee(outcome: ExecutionOutcome) -> FeeSettlementReceiptV1 {
    FeeSettlementReceiptV1::new(FeeSettlementReceiptV1Parts {
        txid: hash(0x11),
        escrow_id: hash(0x22),
        payer: wasm(0x44),
        source_kind: FeeSourceKind::ExecutionBalance,
        outcome,
        base_fee_per_weight: 2,
        max_fee_per_weight: 5,
        max_priority_fee_per_weight: 1,
        max_weight: 100,
        actual_weight: 10,
        effective_price: 3,
        base_component: 20,
        priority_component: 10,
        charged: 30,
        refund: 470,
    })
    .unwrap()
}

fn receipt_parts(outcome: ExecutionReceiptOutcomeV1, trap_code: u16) -> ExecutionReceiptV1Parts {
    let fee = match outcome {
        ExecutionReceiptOutcomeV1::Committed => fee(ExecutionOutcome::Committed),
        ExecutionReceiptOutcomeV1::ResourceExhausted => fee(ExecutionOutcome::ResourceExhausted),
        ExecutionReceiptOutcomeV1::Reverted | ExecutionReceiptOutcomeV1::Trapped => {
            fee(ExecutionOutcome::Reverted)
        }
    };

    ExecutionReceiptV1Parts {
        txid: hash(0x11),
        execution_domain: ExecutionDomain::Wasm,
        outcome,
        trap_code,
        fee_payer: wasm(0x44),
        actual_weight: 10,
        fee_charged: 30,
        fee_settlement_receipt_id: fee.receipt_id(),
        state_effect_root: hash(0x55),
        events_root: hash(0x66),
        event_count: 1,
        return_data_hash: return_data_hash(b"ok"),
        return_data_len: 2,
        outbox_effect_root: empty_outbox_effect_root(),
        outbox_count: 0,
    }
}

#[test]
fn event_exact_structural_limits_are_accepted() {
    let event = ExecutionEventV1::new(
        wasm(1),
        vec![hash(2); MAX_EXECUTION_EVENT_TOPICS_V1],
        vec![3; MAX_EXECUTION_EVENT_DATA_BYTES_V1],
    )
    .unwrap();
    assert_eq!(event.topics().len(), MAX_EXECUTION_EVENT_TOPICS_V1);
    assert_eq!(event.data().len(), MAX_EXECUTION_EVENT_DATA_BYTES_V1);
}

#[test]
fn event_one_over_limits_are_rejected() {
    let too_many_topics = ExecutionEventV1::new(
        wasm(1),
        vec![hash(2); MAX_EXECUTION_EVENT_TOPICS_V1 + 1],
        Vec::new(),
    )
    .unwrap_err();
    assert_eq!(too_many_topics, ExecutionReceiptError::TooManyEventTopics);

    let too_much_data = ExecutionEventV1::new(
        wasm(1),
        Vec::new(),
        vec![3; MAX_EXECUTION_EVENT_DATA_BYTES_V1 + 1],
    )
    .unwrap_err();
    assert_eq!(too_much_data, ExecutionReceiptError::EventDataTooLarge);
}

#[test]
fn events_root_is_order_sensitive() {
    let first = ExecutionEventV1::new(wasm(1), vec![hash(2)], b"a".to_vec()).unwrap();
    let second = ExecutionEventV1::new(wasm(3), vec![hash(4)], b"b".to_vec()).unwrap();
    assert_ne!(
        events_root(&[first.clone(), second.clone()]).unwrap(),
        events_root(&[second, first]).unwrap()
    );
}

#[test]
fn phase_a_effects_reject_receipt_domain_and_noncanonical_order() {
    let receipt_descriptor = StateEffectDescriptorV1::new(
        CommitmentDomainId::ExecutionReceipts,
        CommitmentSchemeId::OregonSmtV1,
        hash(1),
        hash(2),
    )
    .unwrap_err();
    assert_eq!(
        receipt_descriptor,
        ExecutionReceiptError::ReceiptDomainInStateEffects
    );

    let wasm_effect = StateEffectDescriptorV1::new(
        CommitmentDomainId::Wasm,
        CommitmentSchemeId::OregonSmtV1,
        hash(1),
        hash(2),
    )
    .unwrap();
    let evm_effect = StateEffectDescriptorV1::new(
        CommitmentDomainId::Evm,
        CommitmentSchemeId::EvmCommitmentV1,
        hash(1),
        hash(2),
    )
    .unwrap();

    let err = state_effect_root(&[wasm_effect, evm_effect]).unwrap_err();
    assert_eq!(err, ExecutionReceiptError::NonCanonicalEffectOrder);
}

#[test]
fn evm_effect_cannot_masquerade_as_oregon_smt() {
    let err = StateEffectDescriptorV1::new(
        CommitmentDomainId::Evm,
        CommitmentSchemeId::OregonSmtV1,
        hash(1),
        hash(2),
    )
    .unwrap_err();
    assert_eq!(err, ExecutionReceiptError::InvalidEffectScheme);
}

#[test]
fn trapped_receipt_requires_nonzero_trap_code() {
    let fee = fee(ExecutionOutcome::Reverted);
    let err = ExecutionReceiptV1::new(
        receipt_parts(ExecutionReceiptOutcomeV1::Trapped, 0),
        &fee,
    )
    .unwrap_err();
    assert_eq!(err, ExecutionReceiptError::InvalidTrapCode);
}

#[test]
fn non_trapped_receipt_requires_zero_trap_code() {
    let fee = fee(ExecutionOutcome::Committed);
    let err = ExecutionReceiptV1::new(
        receipt_parts(ExecutionReceiptOutcomeV1::Committed, 1),
        &fee,
    )
    .unwrap_err();
    assert_eq!(err, ExecutionReceiptError::InvalidTrapCode);
}

#[test]
fn execution_receipt_cross_checks_fee_truth() {
    let fee = fee(ExecutionOutcome::Committed);
    let mut parts = receipt_parts(ExecutionReceiptOutcomeV1::Committed, 0);
    parts.fee_charged = 31;
    let err = ExecutionReceiptV1::new(parts, &fee).unwrap_err();
    assert_eq!(err, ExecutionReceiptError::FeeReceiptMismatch);
}

#[test]
fn execution_receipt_is_exactly_259_bytes_and_round_trips() {
    let fee = fee(ExecutionOutcome::Committed);
    let receipt = ExecutionReceiptV1::new(
        receipt_parts(ExecutionReceiptOutcomeV1::Committed, 0),
        &fee,
    )
    .unwrap();
    let encoded = receipt.encode();
    assert_eq!(encoded.len(), EXECUTION_RECEIPT_BYTES_V1);
    let decoded = ExecutionReceiptV1::decode(&encoded).unwrap();
    assert_eq!(decoded, receipt);
}
