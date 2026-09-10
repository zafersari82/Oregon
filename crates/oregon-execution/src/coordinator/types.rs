use crate::{JournalError, JournalResultV1};
use oregon_primitives::execution_envelope::ExecutionDomain;
use oregon_primitives::execution_event::MAX_EXECUTION_EVENTS_V1;
use oregon_primitives::execution_receipt::{ExecutionReceiptError, ExecutionReceiptV1};
use oregon_primitives::fee_settlement::FeeSettlementReceiptV1;
use oregon_runtime::{MAX_RUNTIME_CALL_DEPTH, RuntimeTrapCodeV1};
use thiserror::Error;

pub(super) const MAX_COORDINATOR_EFFECT_BYTES_V1: usize = 2_097_152;
pub(super) const MAX_COORDINATOR_FRAMES_V1: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CoordinatorLimitsV1 {
    pub(super) max_call_depth: u16,
    pub(super) max_events: usize,
    pub(super) max_effect_bytes: usize,
}

impl CoordinatorLimitsV1 {
    pub(super) fn new(
        max_call_depth: u16,
        max_events: usize,
        max_effect_bytes: usize,
    ) -> Result<Self, CoordinatorError> {
        if max_call_depth == 0
            || max_events == 0
            || max_effect_bytes == 0
            || max_call_depth > MAX_RUNTIME_CALL_DEPTH
            || max_events > MAX_EXECUTION_EVENTS_V1
            || max_effect_bytes > MAX_COORDINATOR_EFFECT_BYTES_V1
        {
            return Err(CoordinatorError::InvalidLimits);
        }

        Ok(Self {
            max_call_depth,
            max_events,
            max_effect_bytes,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CoordinatorTerminalV1 {
    Running,
    ResourceExhausted,
    Fatal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CoordinatorOutcomeV1 {
    Committed,
    Reverted,
    Trapped(RuntimeTrapCodeV1),
    ResourceExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CoordinatorSettlementV1 {
    outcome: CoordinatorOutcomeV1,
    fee_receipt: FeeSettlementReceiptV1,
    validated_execution_domain: Option<ExecutionDomain>,
}

impl CoordinatorSettlementV1 {
    pub(super) const fn new(
        outcome: CoordinatorOutcomeV1,
        fee_receipt: FeeSettlementReceiptV1,
    ) -> Self {
        Self {
            outcome,
            fee_receipt,
            validated_execution_domain: None,
        }
    }

    pub(super) const fn from_validated(
        outcome: CoordinatorOutcomeV1,
        fee_receipt: FeeSettlementReceiptV1,
        execution_domain: ExecutionDomain,
    ) -> Self {
        Self {
            outcome,
            fee_receipt,
            validated_execution_domain: Some(execution_domain),
        }
    }

    pub(super) const fn outcome(&self) -> CoordinatorOutcomeV1 {
        self.outcome
    }

    pub(super) const fn fee_receipt(&self) -> &FeeSettlementReceiptV1 {
        &self.fee_receipt
    }

    pub(super) const fn validated_execution_domain(&self) -> Option<ExecutionDomain> {
        self.validated_execution_domain
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TransactionExecutionProposalV1 {
    phase_a: JournalResultV1,
    phase_b: JournalResultV1,
    fee_receipt: FeeSettlementReceiptV1,
    execution_receipt: ExecutionReceiptV1,
    execution_fee_total_delta: u64,
}

impl TransactionExecutionProposalV1 {
    pub(super) fn new(
        phase_a: JournalResultV1,
        phase_b: JournalResultV1,
        fee_receipt: FeeSettlementReceiptV1,
        execution_receipt: ExecutionReceiptV1,
        execution_fee_total_delta: u64,
    ) -> Self {
        Self {
            phase_a,
            phase_b,
            fee_receipt,
            execution_receipt,
            execution_fee_total_delta,
        }
    }

    pub(super) const fn phase_a(&self) -> &JournalResultV1 {
        &self.phase_a
    }

    pub(super) const fn phase_b(&self) -> &JournalResultV1 {
        &self.phase_b
    }

    pub(super) const fn fee_receipt(&self) -> &FeeSettlementReceiptV1 {
        &self.fee_receipt
    }

    pub(super) const fn execution_receipt(&self) -> &ExecutionReceiptV1 {
        &self.execution_receipt
    }

    pub(super) const fn execution_fee_total_delta(&self) -> u64 {
        self.execution_fee_total_delta
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(super) enum CoordinatorError {
    #[error("coordinator limits must be positive and within Stage 4B structural ceilings")]
    InvalidLimits,
    #[error("unsupported host charge schedule version {0}")]
    UnsupportedHostChargeScheduleVersion(u16),
    #[error("host charge arithmetic overflowed the V1 u64 weight boundary")]
    HostChargeOverflow,
    #[error("runtime dispatch target must be EVM or WASM")]
    InvalidRuntimeTarget,
    #[error("runtime dispatch table contains a duplicate target")]
    DuplicateRuntimeTarget,
    #[error("execution-funded payer balance is below max escrow")]
    InsufficientExecutionFunding,
    #[error(
        "trusted funding request chain, height or txid does not match the execution journal context"
    )]
    FundingContextMismatch,
    #[error("validated funding capability does not match the trusted funding request")]
    FundingCapabilityMismatch,
    #[error("validated funding capability amount is below max escrow")]
    FundingCapabilityAmountTooSmall,
    #[error("validated funding could not open fee escrow")]
    EscrowOpenFailed,
    #[error("fee escrow settlement failed or was attempted more than once")]
    FeeSettlementFailed,
    #[error("fatal execution state prevents a publishable settlement result")]
    FatalExecution,
    #[error("validated execution domain disagrees with execution receipt domain")]
    ExecutionDomainMismatch,
    #[cfg(not(test))]
    #[error("execution proposal settlement is missing validated execution-domain authority")]
    UnboundExecutionDomain,
    #[error("the root effect frame cannot be ended through child lifecycle operations")]
    RootFrameLifecycle,
    #[error("coordinator call depth limit exceeded")]
    CallDepthExceeded,
    #[error("coordinator total frame creation limit exceeded")]
    FrameLimitExceeded,
    #[error("execution event count exceeds the configured transaction limit")]
    EventLimitExceeded,
    #[error("execution event data exceeds the V1 structural ceiling")]
    EventDataTooLarge,
    #[error("execution event topic count exceeds the V1 structural ceiling")]
    EventTopicsTooLarge,
    #[error("live retained event plus return-data bytes exceed the configured limit")]
    EffectBytesLimitExceeded,
    #[error("coordinator effect accounting invariant violated")]
    AccountingInvariant,
    #[error("journal and effect frame stacks diverged")]
    FrameStackMismatch,
    #[error("ExecutionReceipts cannot participate in Phase-A effects")]
    PhaseAReceiptDomain,
    #[error("Phase-B requires an ExecutionReceipts snapshot")]
    InvalidReceiptSnapshotDomain,
    #[error("fee or execution receipt already exists for the transaction")]
    DuplicateReceipt,
    #[error("invalid execution receipt or state-effect commitment: {0}")]
    ReceiptPrimitive(#[source] ExecutionReceiptError),
    #[error("receipt-state journal failed: {0}")]
    ReceiptState(#[source] JournalError),
}
