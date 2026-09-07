use oregon_contract_state::StateSource;
use oregon_primitives::Hash256;
use oregon_primitives::execution_address::ExecutionAddress;
use oregon_primitives::execution_envelope::ExecutionDomain;
use oregon_primitives::fee_settlement::{ExecutionOutcome, FeeSourceKind};
use oregon_runtime::RuntimeCallResultV1;

use crate::{
    EscrowBookV1, EscrowTicketV1, ExecutionJournalV1, FeeTermsV1, FundingCapabilityV1, WeightMeter,
};

use super::accounting::{
    read_balance, read_total_execution_balance, write_balance, write_total_execution_balance,
};
use super::effects::EffectStackV1;
use super::types::{
    CoordinatorError, CoordinatorOutcomeV1, CoordinatorSettlementV1, CoordinatorTerminalV1,
};

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

pub(super) fn settle_top_level_execution<S: StateSource + ?Sized>(
    journal: &mut ExecutionJournalV1<'_, S>,
    effects: &mut EffectStackV1,
    book: &mut EscrowBookV1,
    ticket: EscrowTicketV1,
    meter: &WeightMeter,
    terminal: &mut CoordinatorTerminalV1,
    runtime_result: RuntimeCallResultV1,
) -> Result<CoordinatorSettlementV1, CoordinatorError> {
    if *terminal == CoordinatorTerminalV1::Fatal {
        let _ = finish_top_level_frames(journal, effects, false, terminal);
        *terminal = CoordinatorTerminalV1::Fatal;
        return Err(CoordinatorError::FatalExecution);
    }

    let exhausted = *terminal == CoordinatorTerminalV1::ResourceExhausted || meter.is_exhausted();
    let (outcome, fee_outcome, commit_child, actual_weight) = if exhausted {
        *terminal = CoordinatorTerminalV1::ResourceExhausted;
        (
            CoordinatorOutcomeV1::ResourceExhausted,
            ExecutionOutcome::ResourceExhausted,
            false,
            meter.max_weight(),
        )
    } else {
        match runtime_result {
            RuntimeCallResultV1::Success(_) => (
                CoordinatorOutcomeV1::Committed,
                ExecutionOutcome::Committed,
                true,
                meter.consumed(),
            ),
            RuntimeCallResultV1::Revert(_) => (
                CoordinatorOutcomeV1::Reverted,
                ExecutionOutcome::Reverted,
                false,
                meter.consumed(),
            ),
            RuntimeCallResultV1::Trap(code) => (
                CoordinatorOutcomeV1::Trapped(code),
                ExecutionOutcome::Reverted,
                false,
                meter.consumed(),
            ),
        }
    };

    finish_top_level_frames(journal, effects, commit_child, terminal)?;

    let mut staged_book = book.clone();
    let settlement = match staged_book.settle(ticket.escrow_id(), fee_outcome, actual_weight) {
        Ok(settlement) => settlement,
        Err(_) => {
            *terminal = CoordinatorTerminalV1::Fatal;
            return Err(CoordinatorError::FeeSettlementFailed);
        }
    };

    if ticket.source_kind() == FeeSourceKind::ExecutionBalance {
        if let Err(error) = apply_execution_funded_settlement(
            journal,
            ticket.payer(),
            settlement.refund(),
            settlement.charged(),
        ) {
            *terminal = CoordinatorTerminalV1::Fatal;
            return Err(error);
        }
    }

    let fee_receipt = *settlement.receipt();
    *book = staged_book;
    Ok(CoordinatorSettlementV1::new(outcome, fee_receipt))
}

fn finish_top_level_frames<S: StateSource + ?Sized>(
    journal: &mut ExecutionJournalV1<'_, S>,
    effects: &mut EffectStackV1,
    commit: bool,
    terminal: &mut CoordinatorTerminalV1,
) -> Result<(), CoordinatorError> {
    let journal_result = if commit {
        journal.commit_frame()
    } else {
        journal.revert_frame()
    };
    let effects_result = if commit {
        effects.commit()
    } else {
        effects.revert()
    };

    if journal_result.is_err() || effects_result.is_err() {
        *terminal = CoordinatorTerminalV1::Fatal;
        return Err(CoordinatorError::FatalExecution);
    }
    Ok(())
}

fn apply_execution_funded_settlement<S: StateSource + ?Sized>(
    journal: &mut ExecutionJournalV1<'_, S>,
    payer: ExecutionAddress,
    refund: u64,
    charged: u64,
) -> Result<(), CoordinatorError> {
    let payer_balance = read_balance(journal, payer)?;
    let next_payer_balance = payer_balance
        .checked_add(refund)
        .ok_or(CoordinatorError::AccountingInvariant)?;
    let total_execution_balance = read_total_execution_balance(journal)?;
    let next_total_execution_balance = total_execution_balance
        .checked_sub(charged)
        .ok_or(CoordinatorError::AccountingInvariant)?;

    write_balance(journal, payer, next_payer_balance)?;
    write_total_execution_balance(journal, next_total_execution_balance)
}
