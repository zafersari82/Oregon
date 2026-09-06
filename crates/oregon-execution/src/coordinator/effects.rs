use oregon_primitives::Hash256;
use oregon_primitives::execution_address::ExecutionAddress;
use oregon_primitives::execution_event::{
    ExecutionEventV1, MAX_EXECUTION_EVENT_DATA_BYTES_V1, MAX_EXECUTION_EVENT_TOPICS_V1,
};

use super::types::{
    CoordinatorError, CoordinatorLimitsV1, CoordinatorTerminalV1, MAX_COORDINATOR_FRAMES_V1,
};

#[derive(Debug, Default)]
pub(super) struct EffectFrameV1 {
    events: Vec<ExecutionEventV1>,
    retained_event_bytes: usize,
    retained_return_bytes: usize,
}

impl EffectFrameV1 {
    fn retained_bytes(&self) -> Result<usize, CoordinatorError> {
        self.retained_event_bytes
            .checked_add(self.retained_return_bytes)
            .ok_or(CoordinatorError::AccountingInvariant)
    }
}

#[derive(Debug)]
pub(super) struct EffectStackV1 {
    frames: Vec<EffectFrameV1>,
    limits: CoordinatorLimitsV1,
    total_frames_created: usize,
    live_event_count: usize,
    live_retained_bytes: usize,
}

impl EffectStackV1 {
    pub(super) fn new(limits: CoordinatorLimitsV1) -> Self {
        Self {
            frames: vec![EffectFrameV1::default()],
            limits,
            total_frames_created: 1,
            live_event_count: 0,
            live_retained_bytes: 0,
        }
    }

    pub(super) fn begin(&mut self) -> Result<(), CoordinatorError> {
        let next_depth = self
            .frames
            .len()
            .checked_add(1)
            .ok_or(CoordinatorError::CallDepthExceeded)?;
        if next_depth > usize::from(self.limits.max_call_depth) {
            return Err(CoordinatorError::CallDepthExceeded);
        }

        let next_total = self
            .total_frames_created
            .checked_add(1)
            .ok_or(CoordinatorError::FrameLimitExceeded)?;
        if next_total > MAX_COORDINATOR_FRAMES_V1 {
            return Err(CoordinatorError::FrameLimitExceeded);
        }

        self.frames.push(EffectFrameV1::default());
        self.total_frames_created = next_total;
        Ok(())
    }

    pub(super) fn commit(&mut self) -> Result<(), CoordinatorError> {
        if self.frames.len() == 1 {
            return Err(CoordinatorError::RootFrameLifecycle);
        }

        let parent_index = self.frames.len() - 2;
        let child_index = self.frames.len() - 1;
        let child = &self.frames[child_index];
        let parent = &self.frames[parent_index];

        let next_parent_event_bytes = parent
            .retained_event_bytes
            .checked_add(child.retained_event_bytes)
            .ok_or(CoordinatorError::AccountingInvariant)?;
        let next_parent_return_bytes = parent
            .retained_return_bytes
            .checked_add(child.retained_return_bytes)
            .ok_or(CoordinatorError::AccountingInvariant)?;

        let child = self
            .frames
            .pop()
            .ok_or(CoordinatorError::AccountingInvariant)?;
        let parent = self
            .frames
            .last_mut()
            .ok_or(CoordinatorError::AccountingInvariant)?;
        parent.events.extend(child.events);
        parent.retained_event_bytes = next_parent_event_bytes;
        parent.retained_return_bytes = next_parent_return_bytes;
        Ok(())
    }

    pub(super) fn revert(&mut self) -> Result<(), CoordinatorError> {
        if self.frames.len() == 1 {
            return Err(CoordinatorError::RootFrameLifecycle);
        }

        let child = self
            .frames
            .last()
            .ok_or(CoordinatorError::AccountingInvariant)?;
        let child_bytes = child.retained_bytes()?;
        let next_event_count = self
            .live_event_count
            .checked_sub(child.events.len())
            .ok_or(CoordinatorError::AccountingInvariant)?;
        let next_retained_bytes = self
            .live_retained_bytes
            .checked_sub(child_bytes)
            .ok_or(CoordinatorError::AccountingInvariant)?;

        self.frames.pop();
        self.live_event_count = next_event_count;
        self.live_retained_bytes = next_retained_bytes;
        Ok(())
    }

    pub(super) fn push_event(&mut self, event: ExecutionEventV1) -> Result<(), CoordinatorError> {
        if event.topics().len() > MAX_EXECUTION_EVENT_TOPICS_V1 {
            return Err(CoordinatorError::EventTopicsTooLarge);
        }
        if event.data().len() > MAX_EXECUTION_EVENT_DATA_BYTES_V1 {
            return Err(CoordinatorError::EventDataTooLarge);
        }
        self.reserve_event(event.data().len())?;

        let frame = self
            .frames
            .last_mut()
            .ok_or(CoordinatorError::AccountingInvariant)?;
        frame.retained_event_bytes = frame
            .retained_event_bytes
            .checked_add(event.data().len())
            .ok_or(CoordinatorError::AccountingInvariant)?;
        frame.events.push(event);
        Ok(())
    }

    pub(super) fn push_event_parts(
        &mut self,
        emitter: ExecutionAddress,
        topics: &[Hash256],
        data: &[u8],
    ) -> Result<(), CoordinatorError> {
        if topics.len() > MAX_EXECUTION_EVENT_TOPICS_V1 {
            return Err(CoordinatorError::EventTopicsTooLarge);
        }
        if data.len() > MAX_EXECUTION_EVENT_DATA_BYTES_V1 {
            return Err(CoordinatorError::EventDataTooLarge);
        }
        self.reserve_event(data.len())?;

        let event = ExecutionEventV1::new(emitter, topics.to_vec(), data.to_vec())
            .map_err(|_| CoordinatorError::AccountingInvariant)?;
        let frame = self
            .frames
            .last_mut()
            .ok_or(CoordinatorError::AccountingInvariant)?;
        frame.retained_event_bytes = frame
            .retained_event_bytes
            .checked_add(data.len())
            .ok_or(CoordinatorError::AccountingInvariant)?;
        frame.events.push(event);
        Ok(())
    }

    pub(super) fn retain_return_bytes(&mut self, bytes: usize) -> Result<(), CoordinatorError> {
        let next_live = self
            .live_retained_bytes
            .checked_add(bytes)
            .ok_or(CoordinatorError::EffectBytesLimitExceeded)?;
        if next_live > self.limits.max_effect_bytes {
            return Err(CoordinatorError::EffectBytesLimitExceeded);
        }

        let frame = self
            .frames
            .last_mut()
            .ok_or(CoordinatorError::AccountingInvariant)?;
        let next_frame = frame
            .retained_return_bytes
            .checked_add(bytes)
            .ok_or(CoordinatorError::AccountingInvariant)?;
        frame.retained_return_bytes = next_frame;
        self.live_retained_bytes = next_live;
        Ok(())
    }

    fn reserve_event(&mut self, data_bytes: usize) -> Result<(), CoordinatorError> {
        let next_count = self
            .live_event_count
            .checked_add(1)
            .ok_or(CoordinatorError::EventLimitExceeded)?;
        if next_count > self.limits.max_events {
            return Err(CoordinatorError::EventLimitExceeded);
        }

        let next_bytes = self
            .live_retained_bytes
            .checked_add(data_bytes)
            .ok_or(CoordinatorError::EffectBytesLimitExceeded)?;
        if next_bytes > self.limits.max_effect_bytes {
            return Err(CoordinatorError::EffectBytesLimitExceeded);
        }

        self.live_event_count = next_count;
        self.live_retained_bytes = next_bytes;
        Ok(())
    }

    pub(super) fn assert_depth_matches(
        &self,
        journal_depth: usize,
        terminal: &mut CoordinatorTerminalV1,
    ) -> Result<(), CoordinatorError> {
        if self.frames.len() != journal_depth {
            *terminal = CoordinatorTerminalV1::Fatal;
            return Err(CoordinatorError::FrameStackMismatch);
        }
        Ok(())
    }

    pub(super) fn depth(&self) -> usize {
        self.frames.len()
    }

    pub(super) fn total_frames_created(&self) -> usize {
        self.total_frames_created
    }

    pub(super) fn live_retained_bytes(&self) -> usize {
        self.live_retained_bytes
    }

    pub(super) fn root_events(&self) -> &[ExecutionEventV1] {
        &self.frames[0].events
    }
}
