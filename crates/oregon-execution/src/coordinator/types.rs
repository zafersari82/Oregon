use oregon_primitives::execution_event::MAX_EXECUTION_EVENTS_V1;
use oregon_runtime::MAX_RUNTIME_CALL_DEPTH;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(super) enum CoordinatorError {
    #[error("coordinator limits must be positive and within Stage 4B structural ceilings")]
    InvalidLimits,
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
}
