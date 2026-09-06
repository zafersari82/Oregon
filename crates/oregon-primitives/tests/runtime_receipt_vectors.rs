use oregon_primitives::execution_address::ExecutionAddress;
use oregon_primitives::execution_effect::{StateEffectDescriptorV1, state_effect_root};
use oregon_primitives::execution_envelope::ExecutionDomain;
use oregon_primitives::execution_event::{ExecutionEventV1, events_root};
use oregon_primitives::execution_receipt::{
    ExecutionReceiptOutcomeV1, ExecutionReceiptV1, ExecutionReceiptV1Parts,
    empty_outbox_effect_root, return_data_hash,
};
use oregon_primitives::fee_settlement::{
    ExecutionOutcome, FeeSettlementReceiptV1, FeeSettlementReceiptV1Parts, FeeSourceKind,
};
use oregon_primitives::state_commitment::{CommitmentDomainId, CommitmentSchemeId};
use oregon_primitives::Hash256;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Corpus {
    event_cases: Vec<EventCase>,
    state_effect_cases: Vec<EffectCase>,
    empty_outbox_root_hex: String,
    receipt_context: ReceiptContext,
    receipt_cases: Vec<ReceiptCase>,
}

#[derive(Debug, Deserialize)]
struct EventCase {
    name: String,
    events: Vec<EventJson>,
    root_hex: String,
}

#[derive(Debug, Deserialize)]
struct EventJson {
    emitter_hex: String,
    topics_hex: Vec<String>,
    data_hex: String,
}

#[derive(Debug, Deserialize)]
struct EffectCase {
    name: String,
    descriptors: Vec<EffectJson>,
    root_hex: String,
}

#[derive(Debug, Deserialize)]
struct EffectJson {
    domain_id: u16,
    scheme_id: u16,
    old_root_hex: String,
    new_root_hex: String,
}

#[derive(Debug, Deserialize)]
struct ReceiptContext {
    txid_hex: String,
    fee_payer_hex: String,
    state_effect_root_hex: String,
    events_root_hex: String,
    event_count: u32,
    actual_weight: u64,
    fee_charged: u64,
    base_fee_per_weight: u64,
    max_fee_per_weight: u64,
    max_priority_fee_per_weight: u64,
    max_weight: u64,
    refund: u64,
}

#[derive(Debug, Deserialize)]
struct ReceiptCase {
    name: String,
    outcome: u8,
    trap_code: u16,
    fee_outcome: u8,
    return_data_hex: String,
    receipt_hex: String,
    receipt_id_hex: String,
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    (0..value.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&value[offset..offset + 2], 16).unwrap())
        .collect()
}

fn hash_hex(value: &str) -> Hash256 {
    Hash256::from_slice(&decode_hex(value)).unwrap()
}

fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../../../tests/vectors/runtime-coordinator-v1.json")).unwrap()
}

fn fee(context: &ReceiptContext, outcome: ExecutionOutcome) -> FeeSettlementReceiptV1 {
    let max_escrow = context.max_weight * context.max_fee_per_weight;
    let effective_price = context
        .max_fee_per_weight
        .min(context.base_fee_per_weight + context.max_priority_fee_per_weight);
    let base_component = context.actual_weight * context.base_fee_per_weight;
    let charged = context.actual_weight * effective_price;
    let priority_component = charged - base_component;

    FeeSettlementReceiptV1::new(FeeSettlementReceiptV1Parts {
        txid: hash_hex(&context.txid_hex),
        escrow_id: Hash256::from_slice(&[0x22; 32]).unwrap(),
        payer: ExecutionAddress::from_slice(&decode_hex(&context.fee_payer_hex)).unwrap(),
        source_kind: FeeSourceKind::ExecutionBalance,
        outcome,
        base_fee_per_weight: context.base_fee_per_weight,
        max_fee_per_weight: context.max_fee_per_weight,
        max_priority_fee_per_weight: context.max_priority_fee_per_weight,
        max_weight: context.max_weight,
        actual_weight: context.actual_weight,
        effective_price,
        base_component,
        priority_component,
        charged,
        refund: max_escrow - charged,
    })
    .unwrap()
}

fn receipt_outcome(value: u8) -> ExecutionReceiptOutcomeV1 {
    match value {
        0 => ExecutionReceiptOutcomeV1::Committed,
        1 => ExecutionReceiptOutcomeV1::Reverted,
        2 => ExecutionReceiptOutcomeV1::Trapped,
        3 => ExecutionReceiptOutcomeV1::ResourceExhausted,
        other => panic!("unknown vector execution outcome {other}"),
    }
}

fn fee_outcome(value: u8) -> ExecutionOutcome {
    match value {
        0 => ExecutionOutcome::Committed,
        1 => ExecutionOutcome::Reverted,
        2 => ExecutionOutcome::ResourceExhausted,
        other => panic!("unknown vector fee outcome {other}"),
    }
}

#[test]
fn event_roots_match_independent_vectors() {
    for case in corpus().event_cases {
        let events: Vec<_> = case
            .events
            .into_iter()
            .map(|event| {
                let emitter = ExecutionAddress::from_slice(&decode_hex(&event.emitter_hex)).unwrap();
                let topics = event
                    .topics_hex
                    .iter()
                    .map(|topic| hash_hex(topic))
                    .collect();
                ExecutionEventV1::new(emitter, topics, decode_hex(&event.data_hex)).unwrap()
            })
            .collect();
        assert_eq!(
            events_root(&events).unwrap(),
            hash_hex(&case.root_hex),
            "{}",
            case.name
        );
    }
}

#[test]
fn state_effect_roots_match_independent_vectors() {
    for case in corpus().state_effect_cases {
        let descriptors: Vec<_> = case
            .descriptors
            .into_iter()
            .map(|descriptor| {
                StateEffectDescriptorV1::new(
                    CommitmentDomainId::try_from(descriptor.domain_id).unwrap(),
                    CommitmentSchemeId::try_from(descriptor.scheme_id).unwrap(),
                    hash_hex(&descriptor.old_root_hex),
                    hash_hex(&descriptor.new_root_hex),
                )
                .unwrap()
            })
            .collect();
        assert_eq!(
            state_effect_root(&descriptors).unwrap(),
            hash_hex(&case.root_hex),
            "{}",
            case.name
        );
    }
}

#[test]
fn empty_outbox_root_matches_independent_vector() {
    let expected = hash_hex(&corpus().empty_outbox_root_hex);
    assert_eq!(empty_outbox_effect_root(), expected);
}

#[test]
fn all_four_receipts_match_independent_byte_vectors() {
    let corpus = corpus();
    let context = &corpus.receipt_context;
    assert_eq!(context.refund, 470);

    for case in corpus.receipt_cases {
        let fee = fee(context, fee_outcome(case.fee_outcome));
        let return_data = decode_hex(&case.return_data_hex);
        let parts = ExecutionReceiptV1Parts {
            txid: hash_hex(&context.txid_hex),
            execution_domain: ExecutionDomain::Wasm,
            outcome: receipt_outcome(case.outcome),
            trap_code: case.trap_code,
            fee_payer: ExecutionAddress::from_slice(&decode_hex(&context.fee_payer_hex)).unwrap(),
            actual_weight: context.actual_weight,
            fee_charged: context.fee_charged,
            fee_settlement_receipt_id: fee.receipt_id(),
            state_effect_root: hash_hex(&context.state_effect_root_hex),
            events_root: hash_hex(&context.events_root_hex),
            event_count: context.event_count,
            return_data_hash: return_data_hash(&return_data),
            return_data_len: u32::try_from(return_data.len()).unwrap(),
            outbox_effect_root: empty_outbox_effect_root(),
            outbox_count: 0,
        };
        let receipt = ExecutionReceiptV1::new(parts, &fee).unwrap();

        assert_eq!(
            receipt.encode().as_slice(),
            decode_hex(&case.receipt_hex).as_slice(),
            "{} bytes",
            case.name
        );
        assert_eq!(
            receipt.receipt_id(),
            hash_hex(&case.receipt_id_hex),
            "{} id",
            case.name
        );
        assert_eq!(
            ExecutionReceiptV1::decode(&receipt.encode()).unwrap(),
            receipt,
            "{} round trip",
            case.name
        );
    }
}
