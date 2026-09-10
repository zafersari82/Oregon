use oregon_contract_state::{
    DomainSnapshot, StateError, StateNode, StateSource, empty_hashes, encode_accounting_u64,
    total_execution_balance_key,
};
use oregon_primitives::Hash256;
use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::execution_envelope::ExecutionDomain;
use oregon_primitives::execution_event::MAX_EXECUTION_EVENTS_V1;
use oregon_primitives::fee_settlement::FeeSourceKind;
use oregon_primitives::state_commitment::CommitmentDomainId;
use oregon_runtime::RuntimeCallResultV1;

use crate::{
    EscrowBookV1, ExecutionJournalV1, FeeTermsV1, FundingCapabilityV1, JournalContextV1,
    JournalIntentV1, JournalLimitsV1, MeterScheduleV1, WeightMeter, WeightRatio,
};

use super::accounting::write_balance;
use super::effects::EffectStackV1;
use super::proposal::compose_transaction_execution_proposal;
use super::settlement::{
    FundingSourceValidatorV1, FundingValidationRequestV1, open_bound_validated_escrow,
    settle_bound_top_level_execution,
};
use super::types::{CoordinatorError, CoordinatorLimitsV1, CoordinatorTerminalV1};

#[derive(Debug, Default)]
struct EmptySource;

impl StateSource for EmptySource {
    fn get_node(&self, _node_hash: &Hash256) -> Result<Option<StateNode>, StateError> {
        Ok(None)
    }

    fn get_value(&self, _value_hash: &Hash256) -> Result<Option<Vec<u8>>, StateError> {
        Ok(None)
    }
}

fn address(kind: ExecutionAddressKind, byte: u8) -> ExecutionAddress {
    match kind {
        ExecutionAddressKind::Evm => ExecutionAddress::from_evm([byte; 20]),
        _ => ExecutionAddress::new(kind, [byte; 32]).unwrap(),
    }
}

fn payer() -> ExecutionAddress {
    address(ExecutionAddressKind::Wasm, 0x77)
}

fn journal(source: &EmptySource) -> ExecutionJournalV1<'_, EmptySource> {
    let domain = CommitmentDomainId::ExecutionAccounting;
    ExecutionJournalV1::new(
        source,
        JournalContextV1 {
            chain_id: 42,
            height: 9001,
            parent_block_hash: Hash256::from_bytes([0x11; 32]),
            txid: Hash256::from_bytes([0x22; 32]),
        },
        &[DomainSnapshot {
            domain,
            root: empty_hashes(domain)[0],
        }],
        JournalLimitsV1::default(),
    )
    .unwrap()
}

fn request() -> FundingValidationRequestV1 {
    FundingValidationRequestV1 {
        chain_id: 42,
        height: 9001,
        txid: Hash256::from_bytes([0x22; 32]),
        execution_domain: ExecutionDomain::Wasm,
        principal: address(ExecutionAddressKind::Wasm, 0x66),
        payer: payer(),
        authorization_commitment: Hash256::from_bytes([0x33; 32]),
        source_kind: FeeSourceKind::ExecutionBalance,
        source_commitment: Hash256::from_bytes([0x44; 32]),
        source_sequence: 7,
        fee_terms: FeeTermsV1::new(1, 4, 1, 10).unwrap(),
    }
}

struct Validator(FundingCapabilityV1);

impl FundingSourceValidatorV1 for Validator {
    fn validate(
        &mut self,
        _request: &FundingValidationRequestV1,
    ) -> Result<FundingCapabilityV1, CoordinatorError> {
        Ok(self.0)
    }
}

fn validator() -> Validator {
    let request = request();
    Validator(
        FundingCapabilityV1::new(
            request.source_kind,
            request.payer,
            request.source_commitment,
            100,
            request.source_sequence,
            request.authorization_commitment,
        )
        .unwrap(),
    )
}

fn meter() -> WeightMeter {
    let schedule = MeterScheduleV1::new(
        1,
        WeightRatio::new(1, 1).unwrap(),
        WeightRatio::new(1, 1).unwrap(),
        WeightRatio::new(1, 1).unwrap(),
    )
    .unwrap();
    WeightMeter::new(schedule, 10, 5).unwrap()
}

fn effects() -> EffectStackV1 {
    EffectStackV1::new(
        CoordinatorLimitsV1::new(64, MAX_EXECUTION_EVENTS_V1, 2_097_152).unwrap(),
    )
}

#[test]
fn validated_wasm_domain_rejects_evm_receipt_label() {
    let source = EmptySource;
    let mut journal = journal(&source);
    write_balance(&mut journal, payer(), 100).unwrap();
    journal
        .put(
            CommitmentDomainId::ExecutionAccounting,
            total_execution_balance_key(),
            &encode_accounting_u64(100),
        )
        .unwrap();

    let mut validator = validator();
    let mut book = EscrowBookV1::new(4).unwrap();
    let escrow = open_bound_validated_escrow(
        &mut journal,
        &mut book,
        &mut validator,
        request(),
    )
    .unwrap();

    let mut effects = effects();
    journal.begin_frame().unwrap();
    effects.begin().unwrap();
    let mut terminal = CoordinatorTerminalV1::Running;
    let settlement = settle_bound_top_level_execution(
        &mut journal,
        &mut effects,
        &mut book,
        escrow,
        &meter(),
        &mut terminal,
        RuntimeCallResultV1::Success(Vec::new()),
    )
    .unwrap();
    let phase_a = journal.finalize(JournalIntentV1::Committed).unwrap();

    let receipt_source = EmptySource;
    let receipt_domain = CommitmentDomainId::ExecutionReceipts;
    let result = compose_transaction_execution_proposal(
        phase_a,
        &receipt_source,
        DomainSnapshot {
            domain: receipt_domain,
            root: empty_hashes(receipt_domain)[0],
        },
        settlement,
        ExecutionDomain::Evm,
        effects.root_events(),
        b"",
    );

    assert_eq!(result, Err(CoordinatorError::ExecutionDomainMismatch));
}
