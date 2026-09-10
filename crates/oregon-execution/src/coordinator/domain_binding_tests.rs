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
use oregon_runtime::{
    RuntimeBackendFailureV1, RuntimeBackendV1, RuntimeCallContextV1, RuntimeCallContextV1Parts,
    RuntimeCallResultV1, RuntimeHostV1,
};

use crate::{
    EscrowBookV1, ExecutionJournalV1, FeeTermsV1, FundingCapabilityV1, JournalContextV1,
    JournalIntentV1, JournalLimitsV1, MeterScheduleV1, WeightMeter, WeightRatio,
};

use super::accounting::write_balance;
use super::calls::RuntimeDispatchTableV1;
use super::effects::EffectStackV1;
use super::host::{HostChargeScheduleV1, HostChargeScheduleV1Parts};
use super::proposal::compose_transaction_execution_proposal;
use super::settlement::{
    FundingSourceValidatorV1, FundingValidationRequestV1, open_bound_validated_escrow,
    settle_bound_top_level_execution,
};
use super::types::{CoordinatorError, CoordinatorLimitsV1, CoordinatorTerminalV1};
use super::execute_transaction_v1;

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

fn target() -> ExecutionAddress {
    address(ExecutionAddressKind::Wasm, 0x88)
}

fn journal(source: &EmptySource) -> ExecutionJournalV1<'_, EmptySource> {
    let accounting = CommitmentDomainId::ExecutionAccounting;
    let wasm = CommitmentDomainId::Wasm;
    ExecutionJournalV1::new(
        source,
        JournalContextV1 {
            chain_id: 42,
            height: 9001,
            parent_block_hash: Hash256::from_bytes([0x11; 32]),
            txid: Hash256::from_bytes([0x22; 32]),
        },
        &[
            DomainSnapshot {
                domain: accounting,
                root: empty_hashes(accounting)[0],
            },
            DomainSnapshot {
                domain: wasm,
                root: empty_hashes(wasm)[0],
            },
        ],
        JournalLimitsV1::default(),
    )
    .unwrap()
}

fn seeded_journal(source: &EmptySource) -> ExecutionJournalV1<'_, EmptySource> {
    let mut journal = journal(source);
    write_balance(&mut journal, payer(), 100).unwrap();
    journal
        .put(
            CommitmentDomainId::ExecutionAccounting,
            total_execution_balance_key(),
            &encode_accounting_u64(100),
        )
        .unwrap();
    journal
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
    EffectStackV1::new(limits())
}

fn limits() -> CoordinatorLimitsV1 {
    CoordinatorLimitsV1::new(64, MAX_EXECUTION_EVENTS_V1, 2_097_152).unwrap()
}

fn charges() -> HostChargeScheduleV1 {
    HostChargeScheduleV1::new(HostChargeScheduleV1Parts {
        version: 1,
        state_read_base: 1,
        state_read_key_byte: 1,
        state_read_copy_byte: 1,
        state_write_base: 1,
        state_write_key_byte: 1,
        state_write_value_byte: 1,
        state_delete_base: 1,
        state_delete_key_byte: 1,
        event_base: 1,
        event_topic: 1,
        event_data_byte: 1,
        nested_call_base: 1,
        nested_call_input_byte: 1,
        nested_call_return_copy_byte: 1,
        context_query: 1,
    })
    .unwrap()
}

fn top_level_context() -> RuntimeCallContextV1 {
    let request = request();
    RuntimeCallContextV1::from_trusted_parts(RuntimeCallContextV1Parts {
        chain_id: u64::from(request.chain_id),
        height: request.height,
        parent_block_hash: Hash256::from_bytes([0x11; 32]),
        txid: request.txid,
        principal: request.principal,
        caller: request.principal,
        target: target(),
        execution_domain: request.execution_domain,
        depth: 1,
        read_only: false,
        transferred_value: 0,
    })
    .unwrap()
}

struct OwnerBackend;

impl RuntimeBackendV1 for OwnerBackend {
    fn execute(
        &mut self,
        host: &mut dyn RuntimeHostV1,
    ) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1> {
        host.state_put(b"owner-key", b"owner-value")
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        host.emit_event(&[Hash256::from_bytes([0xaa; 32])], b"owner-event")
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        RuntimeCallResultV1::success(b"ok".to_vec()).map_err(|_| RuntimeBackendFailureV1::Fatal)
    }
}

fn owner_backend() -> Box<dyn RuntimeBackendV1> {
    Box::new(OwnerBackend)
}

fn dispatch() -> RuntimeDispatchTableV1 {
    RuntimeDispatchTableV1::new(&[(target(), owner_backend)]).unwrap()
}

fn receipt_snapshot() -> DomainSnapshot {
    let domain = CommitmentDomainId::ExecutionReceipts;
    DomainSnapshot {
        domain,
        root: empty_hashes(domain)[0],
    }
}

#[test]
fn validated_wasm_domain_rejects_evm_receipt_label() {
    let source = EmptySource;
    let mut journal = seeded_journal(&source);

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
    let result = compose_transaction_execution_proposal(
        phase_a,
        &receipt_source,
        receipt_snapshot(),
        settlement,
        ExecutionDomain::Evm,
        effects.root_events(),
        b"",
    );

    assert_eq!(result, Err(CoordinatorError::ExecutionDomainMismatch));
}

#[test]
fn owning_path_composes_backend_effects_phase_a_settlement_and_phase_b() {
    let source = EmptySource;
    let receipt_source = EmptySource;
    let mut validator = validator();
    let mut book = EscrowBookV1::new(4).unwrap();

    let proposal = execute_transaction_v1(
        seeded_journal(&source),
        &receipt_source,
        receipt_snapshot(),
        &mut book,
        &mut validator,
        request(),
        top_level_context(),
        meter(),
        limits(),
        charges(),
        &dispatch(),
    )
    .unwrap();

    assert_eq!(
        proposal.execution_receipt().parts().execution_domain,
        ExecutionDomain::Wasm
    );
    assert_eq!(proposal.execution_receipt().parts().event_count, 1);
    assert_eq!(proposal.execution_receipt().parts().return_data_len, 2);
    assert!(proposal.phase_a().roots.iter().any(|roots| {
        roots.domain == CommitmentDomainId::Wasm && roots.old_root != roots.new_root
    }));
    assert_eq!(proposal.phase_b().roots.len(), 1);
    assert_eq!(
        proposal.phase_b().roots[0].domain,
        CommitmentDomainId::ExecutionReceipts
    );
    assert_eq!(proposal.execution_fee_total_delta(), proposal.fee_receipt().charged());
}

#[test]
fn phase_b_failure_cannot_consume_external_escrow_book_in_owning_path() {
    let source = EmptySource;
    let receipt_source = EmptySource;
    let mut validator = validator();
    let mut book = EscrowBookV1::new(4).unwrap();
    let corrupt_snapshot = DomainSnapshot {
        domain: CommitmentDomainId::ExecutionReceipts,
        root: Hash256::from_bytes([0x99; 32]),
    };

    let first = execute_transaction_v1(
        seeded_journal(&source),
        &receipt_source,
        corrupt_snapshot,
        &mut book,
        &mut validator,
        request(),
        top_level_context(),
        meter(),
        limits(),
        charges(),
        &dispatch(),
    );
    assert!(matches!(first, Err(CoordinatorError::ReceiptState(_))));

    let retry = execute_transaction_v1(
        seeded_journal(&source),
        &receipt_source,
        receipt_snapshot(),
        &mut book,
        &mut validator,
        request(),
        top_level_context(),
        meter(),
        limits(),
        charges(),
        &dispatch(),
    );
    assert!(retry.is_ok(), "failed Phase B consumed external escrow authority");
}
