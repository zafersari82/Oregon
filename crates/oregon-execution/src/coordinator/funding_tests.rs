use oregon_contract_state::{
    DomainSnapshot, StateError, StateNode, StateSource, empty_hashes, encode_accounting_u64,
    total_execution_balance_key,
};
use oregon_primitives::Hash256;
use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::execution_envelope::ExecutionDomain;
use oregon_primitives::fee_settlement::FeeSourceKind;
use oregon_primitives::state_commitment::CommitmentDomainId;

use crate::{
    EscrowBookV1, ExecutionJournalV1, FeeTermsV1, FundingCapabilityV1, JournalContextV1,
    JournalLimitsV1,
};

use super::accounting::{read_balance, read_total_execution_balance, write_balance};
use super::settlement::{
    FundingSourceValidatorV1, FundingValidationRequestV1, open_validated_escrow,
};
use super::types::CoordinatorError;

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

fn seed_accounting(journal: &mut ExecutionJournalV1<'_, EmptySource>, value: u64) {
    write_balance(journal, payer(), value).unwrap();
    journal
        .put(
            CommitmentDomainId::ExecutionAccounting,
            total_execution_balance_key(),
            &encode_accounting_u64(value),
        )
        .unwrap();
}

fn terms() -> FeeTermsV1 {
    FeeTermsV1::new(1, 4, 1, 10).unwrap()
}

fn request(source_kind: FeeSourceKind) -> FundingValidationRequestV1 {
    FundingValidationRequestV1 {
        chain_id: 42,
        height: 9001,
        txid: Hash256::from_bytes([0x22; 32]),
        execution_domain: ExecutionDomain::Wasm,
        principal: address(ExecutionAddressKind::Wasm, 0x66),
        payer: payer(),
        authorization_commitment: Hash256::from_bytes([0x33; 32]),
        source_kind,
        source_commitment: Hash256::from_bytes([0x44; 32]),
        source_sequence: 7,
        fee_terms: terms(),
    }
}

fn capability(
    source_kind: FeeSourceKind,
    capability_payer: ExecutionAddress,
    source_commitment: Hash256,
    available_amount: u64,
    source_sequence: u64,
    authorization_commitment: Hash256,
) -> FundingCapabilityV1 {
    FundingCapabilityV1::new(
        source_kind,
        capability_payer,
        source_commitment,
        available_amount,
        source_sequence,
        authorization_commitment,
    )
    .unwrap()
}

#[derive(Debug)]
struct RecordingValidator {
    capability: FundingCapabilityV1,
    seen: Option<FundingValidationRequestV1>,
}

impl FundingSourceValidatorV1 for RecordingValidator {
    fn validate(
        &mut self,
        request: &FundingValidationRequestV1,
    ) -> Result<FundingCapabilityV1, CoordinatorError> {
        self.seen = Some(*request);
        Ok(self.capability)
    }
}

fn matching_validator(source_kind: FeeSourceKind, available_amount: u64) -> RecordingValidator {
    let request = request(source_kind);
    RecordingValidator {
        capability: capability(
            source_kind,
            request.payer,
            request.source_commitment,
            available_amount,
            request.source_sequence,
            request.authorization_commitment,
        ),
        seen: None,
    }
}

#[test]
fn validated_execution_capability_binds_full_request_and_reserves_root_balance() {
    let source = EmptySource;
    let mut journal = journal(&source);
    seed_accounting(&mut journal, 100);
    let request = request(FeeSourceKind::ExecutionBalance);
    let mut validator = matching_validator(FeeSourceKind::ExecutionBalance, 100);
    let mut book = EscrowBookV1::new(4).unwrap();

    let ticket = open_validated_escrow(&mut journal, &mut book, &mut validator, request).unwrap();

    assert_eq!(validator.seen, Some(request));
    assert_eq!(ticket.txid(), request.txid);
    assert_eq!(ticket.payer(), request.payer);
    assert_eq!(ticket.source_kind(), request.source_kind);
    assert_eq!(ticket.terms(), request.fee_terms);
    assert_eq!(read_balance(&journal, payer()).unwrap(), 60);
    assert_eq!(read_total_execution_balance(&journal).unwrap(), 100);
}

#[test]
fn capability_identity_mismatch_is_rejected_before_book_open_or_root_debit() {
    let cases = [
        capability(
            FeeSourceKind::ExecutionBalance,
            address(ExecutionAddressKind::Wasm, 0x78),
            Hash256::from_bytes([0x44; 32]),
            100,
            7,
            Hash256::from_bytes([0x33; 32]),
        ),
        capability(
            FeeSourceKind::NativeUtxo,
            payer(),
            Hash256::from_bytes([0x44; 32]),
            100,
            7,
            Hash256::from_bytes([0x33; 32]),
        ),
        capability(
            FeeSourceKind::ExecutionBalance,
            payer(),
            Hash256::from_bytes([0x45; 32]),
            100,
            7,
            Hash256::from_bytes([0x33; 32]),
        ),
        capability(
            FeeSourceKind::ExecutionBalance,
            payer(),
            Hash256::from_bytes([0x44; 32]),
            100,
            8,
            Hash256::from_bytes([0x33; 32]),
        ),
        capability(
            FeeSourceKind::ExecutionBalance,
            payer(),
            Hash256::from_bytes([0x44; 32]),
            100,
            7,
            Hash256::from_bytes([0x34; 32]),
        ),
    ];

    for mismatched in cases {
        let source = EmptySource;
        let mut journal = journal(&source);
        seed_accounting(&mut journal, 100);
        let request = request(FeeSourceKind::ExecutionBalance);
        let mut validator = RecordingValidator {
            capability: mismatched,
            seen: None,
        };
        let mut book = EscrowBookV1::new(4).unwrap();

        assert_eq!(
            open_validated_escrow(&mut journal, &mut book, &mut validator, request),
            Err(CoordinatorError::FundingCapabilityMismatch)
        );
        assert_eq!(read_balance(&journal, payer()).unwrap(), 100);
        assert_eq!(read_total_execution_balance(&journal).unwrap(), 100);

        let mut valid = matching_validator(FeeSourceKind::ExecutionBalance, 100);
        assert!(open_validated_escrow(&mut journal, &mut book, &mut valid, request).is_ok());
    }
}

#[test]
fn insufficient_validated_amount_is_rejected_before_root_debit() {
    let source = EmptySource;
    let mut journal = journal(&source);
    seed_accounting(&mut journal, 100);
    let request = request(FeeSourceKind::ExecutionBalance);
    let mut validator = matching_validator(FeeSourceKind::ExecutionBalance, 39);
    let mut book = EscrowBookV1::new(4).unwrap();

    assert_eq!(
        open_validated_escrow(&mut journal, &mut book, &mut validator, request),
        Err(CoordinatorError::FundingCapabilityAmountTooSmall)
    );
    assert_eq!(read_balance(&journal, payer()).unwrap(), 100);
    assert_eq!(read_total_execution_balance(&journal).unwrap(), 100);
}

#[test]
fn native_funded_escrow_never_debits_execution_accounting() {
    let source = EmptySource;
    let mut journal = journal(&source);
    seed_accounting(&mut journal, 100);
    let request = request(FeeSourceKind::NativeUtxo);
    let mut validator = matching_validator(FeeSourceKind::NativeUtxo, 100);
    let mut book = EscrowBookV1::new(4).unwrap();

    assert!(open_validated_escrow(&mut journal, &mut book, &mut validator, request).is_ok());
    assert_eq!(read_balance(&journal, payer()).unwrap(), 100);
    assert_eq!(read_total_execution_balance(&journal).unwrap(), 100);
}
