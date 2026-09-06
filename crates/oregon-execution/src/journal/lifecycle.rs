use oregon_contract_state::StateSource;

use super::ExecutionJournalV1;
use super::frame::JournalFrame;
use super::types::JournalError;

impl<S: StateSource + ?Sized> ExecutionJournalV1<'_, S> {
    pub fn begin_frame(&mut self) -> Result<(), JournalError> {
        let next_depth = self
            .frames
            .len()
            .checked_add(1)
            .ok_or(JournalError::DepthLimitExceeded)?;
        if next_depth > self.limits.max_depth {
            return Err(JournalError::DepthLimitExceeded);
        }

        let next_total = self
            .total_frames_created
            .checked_add(1)
            .ok_or(JournalError::FrameLimitExceeded)?;
        if next_total > self.limits.max_frames {
            return Err(JournalError::FrameLimitExceeded);
        }

        self.frames.push(JournalFrame::default());
        self.total_frames_created = next_total;
        Ok(())
    }

    pub fn revert_frame(&mut self) -> Result<(), JournalError> {
        if self.frames.len() == 1 {
            return Err(JournalError::RootFrameLifecycle);
        }

        let child = self
            .frames
            .last()
            .ok_or(JournalError::AccountingInvariant)?;
        let next_live_entries = self
            .live_entries
            .checked_sub(child.entries)
            .ok_or(JournalError::AccountingInvariant)?;
        let next_retained_bytes = self
            .retained_bytes
            .checked_sub(child.retained_bytes)
            .ok_or(JournalError::AccountingInvariant)?;

        self.frames.pop();
        self.live_entries = next_live_entries;
        self.retained_bytes = next_retained_bytes;
        Ok(())
    }
}
