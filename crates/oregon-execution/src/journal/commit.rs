use oregon_contract_state::StateSource;

use super::ExecutionJournalV1;
use super::frame::JournalFrame;
use super::types::JournalError;

impl<'a, S: StateSource + ?Sized> ExecutionJournalV1<'a, S> {
    pub fn commit_frame(&mut self) -> Result<(), JournalError> {
        if self.frames.len() == 1 {
            return Err(JournalError::RootFrameLifecycle);
        }

        let parent_index = self.frames.len() - 2;
        let child_index = self.frames.len() - 1;
        let (replaced_entries, replaced_bytes) = {
            let parent = &self.frames[parent_index];
            let child = &self.frames[child_index];
            let mut entries = 0usize;
            let mut bytes = 0usize;

            for (domain, child_writes) in &child.writes {
                let Some(parent_writes) = parent.writes.get(domain) else {
                    continue;
                };
                for key in child_writes.keys() {
                    let Some(parent_value) = parent_writes.get(key.as_slice()) else {
                        continue;
                    };
                    entries = entries
                        .checked_add(1)
                        .ok_or(JournalError::AccountingInvariant)?;
                    bytes = bytes
                        .checked_add(JournalFrame::entry_bytes(key.len(), parent_value))
                        .ok_or(JournalError::AccountingInvariant)?;
                }
            }
            (entries, bytes)
        };

        let parent = &self.frames[parent_index];
        let child = &self.frames[child_index];
        let next_parent_entries = parent
            .entries
            .checked_add(child.entries)
            .and_then(|value| value.checked_sub(replaced_entries))
            .ok_or(JournalError::AccountingInvariant)?;
        let next_parent_bytes = parent
            .retained_bytes
            .checked_add(child.retained_bytes)
            .and_then(|value| value.checked_sub(replaced_bytes))
            .ok_or(JournalError::AccountingInvariant)?;
        let next_live_entries = self
            .live_entries
            .checked_sub(replaced_entries)
            .ok_or(JournalError::AccountingInvariant)?;
        let next_retained_bytes = self
            .retained_bytes
            .checked_sub(replaced_bytes)
            .ok_or(JournalError::AccountingInvariant)?;

        let child = self.frames.pop().ok_or(JournalError::AccountingInvariant)?;
        let parent = self
            .frames
            .last_mut()
            .ok_or(JournalError::AccountingInvariant)?;
        for (domain, child_writes) in child.writes {
            parent
                .writes
                .entry(domain)
                .or_default()
                .extend(child_writes);
        }
        parent.entries = next_parent_entries;
        parent.retained_bytes = next_parent_bytes;
        self.live_entries = next_live_entries;
        self.retained_bytes = next_retained_bytes;
        Ok(())
    }
}
