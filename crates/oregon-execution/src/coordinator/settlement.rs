use oregon_contract_state::StateSource;
use oregon_primitives::Hash256;
use oregon_primitives::execution_address::ExecutionAddress;
use oregon_primitives::execution_envelope::ExecutionDomain;
use oregon_primitives::fee_settlement::FeeSourceKind;

use crate::{EscrowBookV1, EscrowTicketV1, ExecutionJournalV1, FeeTermsV1, FundingCapabilityV1};

use super::accounting::{read_balance, write_balance};
use super::types::CoordinatorError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FundingValidationRequestV1 {
    pub(super) chain_id: u32,
    pub(super) height: u64,
    pub(super) txid: Hash256,
    pub(super) execution_domain: ExecutionDomain,
    pub(super) principal: ExecutionAddress,
    pub(super) payer: ExecutionAddress,
    pub(super) authorization_commitment: Hash256,
    pub(super) source_kind: FeeSourceKind,
    pub(super) source_commitment: Hash256,
    pub(super) source_sequence: u64,
    pub(super) fee_terms: FeeTermsV1,
}

pub(super) trait FundingSourceValidatorV1 {
    fn validate(
        &mut self,
        request: &FundingValidationRequestV1,
    ) -> Result<FundingCapabilityV1, CoordinatorError>;
}

pub(super) fn open_validated_escrow<S, V>(
    journal: &mut ExecutionJournalV1<'_, S>,
    book: &mut EscrowBookV1,
    validator: &mut V,
    request: FundingValidationRequestV1,
) -> Result<EscrowTicketV1, CoordinatorError>
where
    S: StateSource + ?Sized,
    V: FundingSourceValidatorV1 + ?Sized,
{
    let capability = validator.validate(&request)?;

    if capability.source_kind() != request.source_kind
        || capability.payer() != request.payer
        || capability.source_commitment() != request.source_commitment
        || capability.source_sequence() != request.source_sequence
        || capability.authorization_commitment() != request.authorization_commitment
    {
        return Err(CoordinatorError::FundingCapabilityMismatch);
    }

    if capability.available_amount() < request.fee_terms.max_escrow() {
        return Err(CoordinatorError::FundingCapabilityAmountTooSmall);
    }

    let mut staged_book = book.clone();
    let ticket = staged_book
        .open(request.txid, capability, request.fee_terms)
        .map_err(|_| CoordinatorError::EscrowOpenFailed)?;

    match request.source_kind {
        FeeSourceKind::ExecutionBalance => {
            reserve_execution_funded_escrow(
                journal,
                request.payer,
                request.fee_terms.max_escrow(),
            )?;
        }
        FeeSourceKind::NativeUtxo => {}
    }

    *book = staged_book;
    Ok(ticket)
}

pub(super) fn reserve_execution_funded_escrow<S: StateSource + ?Sized>(
    journal: &mut ExecutionJournalV1<'_, S>,
    payer: ExecutionAddress,
    max_escrow: u64,
) -> Result<(), CoordinatorError> {
    let current = read_balance(journal, payer)?;
    let available = current
        .checked_sub(max_escrow)
        .ok_or(CoordinatorError::InsufficientExecutionFunding)?;
    write_balance(journal, payer, available)
}
