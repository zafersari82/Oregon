use oregon_contract_state::{DomainSnapshot, StateSource};
use oregon_primitives::Hash256;
use oregon_primitives::execution_effect::{StateEffectDescriptorV1, state_effect_root};
use oregon_primitives::execution_envelope::ExecutionDomain;
use oregon_primitives::execution_event::{ExecutionEventV1, events_root};
use oregon_primitives::execution_receipt::{
    ExecutionReceiptError, ExecutionReceiptOutcomeV1, ExecutionReceiptV1,
    ExecutionReceiptV1Parts, empty_outbox_effect_root, return_data_hash,
};
use oregon_primitives::fee_settlement::FeeSourceKind;
use oregon_primitives::state_commitment::{CommitmentDomainId, CommitmentSchemeId};
use oregon_runtime::{RuntimeCallContextV1, RuntimeCallResultV1, RuntimeTrapCodeV1};

use crate::{
    EscrowBookV1, ExecutionJournalV1, JournalIntentV1, JournalLimitsV1, JournalResultV1,
    WeightMeter,
};

use super::calls::RuntimeDispatchTableV1;
use super::effects::EffectStackV1;
use super::host::{CoordinatorHostV1, HostChargeScheduleV1};
use super::settlement::{
    FundingSourceValidatorV1, FundingValidationRequestV1, open_bound_validated_escrow,
    settle_bound_top_level_execution,
};
use super::types::{
    CoordinatorError, CoordinatorLimitsV1, CoordinatorOutcomeV1, CoordinatorSettlementV1,
    CoordinatorTerminalV1, TransactionExecutionProposalV1,
};

const FEE_RECEIPT_KEY_PREFIX_V1: &[u8] = b"receipt/v1/fee/";
const EXECUTION_RECEIPT_KEY_PREFIX_V1: &[u8] = b"receipt/v1/execution/";

pub(super) fn execute_transaction_v1<S, R, V>(
    mut journal: ExecutionJournalV1<'_, S>,
    receipt_source: &R,
    receipt_snapshot: DomainSnapshot,
    book: &mut EscrowBookV1,
    validator: &mut V,
    request: FundingValidationRequestV1,
    top_level_context: RuntimeCallContextV1,
    mut meter: WeightMeter,
    limits: CoordinatorLimitsV1,
    charges: HostChargeScheduleV1,
    dispatch: &RuntimeDispatchTableV1,
) -> Result<TransactionExecutionProposalV1, CoordinatorError>
where
    S: StateSource + ?Sized,
    R: StateSource + ?Sized,
    V: FundingSourceValidatorV1 + ?Sized,
{
    let mut staged_book = book.clone();
    let escrow = open_bound_validated_escrow(
        &mut journal,
        &mut staged_book,
        validator,
        request,
    )?;

    let mut effects = EffectStackV1::new(limits);
    journal
        .begin_frame()
        .map_err(|_| CoordinatorError::FatalExecution)?;
    if let Err(error) = effects.begin() {
        let _ = journal.revert_frame();
        return Err(error);
    }

    let mut terminal = CoordinatorTerminalV1::Running;
    let mut runtime_result = match dispatch.factory_for(top_level_context.target()) {
        Some(factory) => {
            let backend_result = {
                let mut backend = factory();
                let mut host = CoordinatorHostV1::new_active(
                    &top_level_context,
                    &mut journal,
                    &mut meter,
                    &mut effects,
                    &mut terminal,
                    charges,
                    dispatch,
                );
                backend.execute(&mut host)
            };

            match backend_result {
                Ok(result) => result,
                Err(_) => {
                    terminal = CoordinatorTerminalV1::Fatal;
                    RuntimeCallResultV1::trap(RuntimeTrapCodeV1::BackendDeterministic)
                }
            }
        }
        None => RuntimeCallResultV1::trap(RuntimeTrapCodeV1::InvalidCallTarget),
    };

    if runtime_result.validate().is_err() {
        runtime_result = RuntimeCallResultV1::trap(RuntimeTrapCodeV1::ReturnDataTooLarge);
    }

    let mut return_data = if terminal == CoordinatorTerminalV1::Running {
        runtime_result
            .return_data()
            .map_or_else(Vec::new, <[u8]>::to_vec)
    } else {
        Vec::new()
    };

    if !return_data.is_empty() && effects.retain_return_bytes(return_data.len()).is_err() {
        runtime_result = RuntimeCallResultV1::trap(RuntimeTrapCodeV1::ReturnDataTooLarge);
        return_data.clear();
    }

    let settlement = settle_bound_top_level_execution(
        &mut journal,
        &mut effects,
        &mut staged_book,
        escrow,
        &meter,
        &mut terminal,
        runtime_result,
    )?;

    let phase_a = journal
        .finalize(JournalIntentV1::Committed)
        .map_err(|_| CoordinatorError::FatalExecution)?;
    let proposal = compose_transaction_execution_proposal(
        phase_a,
        receipt_source,
        receipt_snapshot,
        settlement,
        request.execution_domain,
        effects.root_events(),
        &return_data,
    )?;

    *book = staged_book;
    Ok(proposal)
}

pub(super) fn build_phase_a_descriptors(
    phase_a: &JournalResultV1,
) -> Result<Vec<StateEffectDescriptorV1>, CoordinatorError> {
    if phase_a
        .roots
        .iter()
        .any(|roots| roots.domain == CommitmentDomainId::ExecutionReceipts)
    {
        return Err(CoordinatorError::PhaseAReceiptDomain);
    }

    let mut roots = phase_a.roots.clone();
    roots.sort_by_key(|roots| u16::from(roots.domain));
    let descriptors = roots
        .into_iter()
        .map(|roots| {
            StateEffectDescriptorV1::new(
                roots.domain,
                CommitmentSchemeId::OregonSmtV1,
                roots.old_root,
                roots.new_root,
            )
            .map_err(CoordinatorError::ReceiptPrimitive)
        })
        .collect::<Result<Vec<_>, _>>()?;

    state_effect_root(&descriptors).map_err(CoordinatorError::ReceiptPrimitive)?;
    Ok(descriptors)
}

pub(super) fn compose_transaction_execution_proposal<S: StateSource + ?Sized>(
    phase_a: JournalResultV1,
    receipt_source: &S,
    receipt_snapshot: DomainSnapshot,
    settlement: CoordinatorSettlementV1,
    execution_domain: ExecutionDomain,
    events: &[ExecutionEventV1],
    return_data: &[u8],
) -> Result<TransactionExecutionProposalV1, CoordinatorError> {
    if receipt_snapshot.domain != CommitmentDomainId::ExecutionReceipts {
        return Err(CoordinatorError::InvalidReceiptSnapshotDomain);
    }

    if let Some(validated_execution_domain) = settlement.validated_execution_domain() {
        if execution_domain != validated_execution_domain {
            return Err(CoordinatorError::ExecutionDomainMismatch);
        }
    }

    #[cfg(not(test))]
    if settlement.validated_execution_domain().is_none() {
        return Err(CoordinatorError::UnboundExecutionDomain);
    }

    let descriptors = build_phase_a_descriptors(&phase_a)?;
    let effect_root = state_effect_root(&descriptors).map_err(CoordinatorError::ReceiptPrimitive)?;
    let event_root = events_root(events).map_err(CoordinatorError::ReceiptPrimitive)?;
    let event_count = u32::try_from(events.len())
        .map_err(|_| CoordinatorError::ReceiptPrimitive(ExecutionReceiptError::TooManyEvents))?;
    let return_data_len = u32::try_from(return_data.len()).map_err(|_| {
        CoordinatorError::ReceiptPrimitive(ExecutionReceiptError::ReturnDataTooLarge)
    })?;

    let fee_receipt = *settlement.fee_receipt();
    let (outcome, trap_code) = receipt_outcome(settlement.outcome());
    let execution_receipt = ExecutionReceiptV1::new(
        ExecutionReceiptV1Parts {
            txid: phase_a.context.txid,
            execution_domain,
            outcome,
            trap_code,
            fee_payer: fee_receipt.payer(),
            actual_weight: fee_receipt.actual_weight(),
            fee_charged: fee_receipt.charged(),
            fee_settlement_receipt_id: fee_receipt.receipt_id(),
            state_effect_root: effect_root,
            events_root: event_root,
            event_count,
            return_data_hash: return_data_hash(return_data),
            return_data_len,
            outbox_effect_root: empty_outbox_effect_root(),
            outbox_count: 0,
        },
        &fee_receipt,
    )
    .map_err(CoordinatorError::ReceiptPrimitive)?;

    let mut phase_b = ExecutionJournalV1::new(
        receipt_source,
        phase_a.context,
        &[receipt_snapshot],
        JournalLimitsV1::default(),
    )
    .map_err(CoordinatorError::ReceiptState)?;
    let fee_key = receipt_key(FEE_RECEIPT_KEY_PREFIX_V1, phase_a.context.txid);
    let execution_key = receipt_key(EXECUTION_RECEIPT_KEY_PREFIX_V1, phase_a.context.txid);

    let existing_fee = phase_b
        .read(CommitmentDomainId::ExecutionReceipts, &fee_key)
        .map_err(CoordinatorError::ReceiptState)?;
    let existing_execution = phase_b
        .read(CommitmentDomainId::ExecutionReceipts, &execution_key)
        .map_err(CoordinatorError::ReceiptState)?;
    if existing_fee.is_some() || existing_execution.is_some() {
        return Err(CoordinatorError::DuplicateReceipt);
    }

    phase_b
        .put(
            CommitmentDomainId::ExecutionReceipts,
            &fee_key,
            &fee_receipt.encode(),
        )
        .map_err(CoordinatorError::ReceiptState)?;
    phase_b
        .put(
            CommitmentDomainId::ExecutionReceipts,
            &execution_key,
            &execution_receipt.encode(),
        )
        .map_err(CoordinatorError::ReceiptState)?;
    let phase_b = phase_b
        .finalize(JournalIntentV1::Committed)
        .map_err(CoordinatorError::ReceiptState)?;

    let execution_fee_total_delta = match fee_receipt.source_kind() {
        FeeSourceKind::ExecutionBalance => fee_receipt.charged(),
        FeeSourceKind::NativeUtxo => 0,
    };

    Ok(TransactionExecutionProposalV1::new(
        phase_a,
        phase_b,
        fee_receipt,
        execution_receipt,
        execution_fee_total_delta,
    ))
}

const fn receipt_outcome(outcome: CoordinatorOutcomeV1) -> (ExecutionReceiptOutcomeV1, u16) {
    match outcome {
        CoordinatorOutcomeV1::Committed => (ExecutionReceiptOutcomeV1::Committed, 0),
        CoordinatorOutcomeV1::Reverted => (ExecutionReceiptOutcomeV1::Reverted, 0),
        CoordinatorOutcomeV1::Trapped(code) => (ExecutionReceiptOutcomeV1::Trapped, code as u16),
        CoordinatorOutcomeV1::ResourceExhausted => {
            (ExecutionReceiptOutcomeV1::ResourceExhausted, 0)
        }
    }
}

fn receipt_key(prefix: &[u8], txid: Hash256) -> Vec<u8> {
    let mut key = Vec::with_capacity(prefix.len() + 32);
    key.extend_from_slice(prefix);
    key.extend_from_slice(txid.as_bytes());
    key
}
