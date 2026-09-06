use oregon_contract_state::StateSource;
use oregon_primitives::state_commitment::CommitmentDomainId;

use super::ExecutionJournalV1;
use super::frame::{JournalFrame, JournalValue};
use super::types::JournalError;

impl<'a, S: StateSource + ?Sized> ExecutionJournalV1<'a, S> {
    pub(super) fn stage_write(
        &mut self,
        domain: CommitmentDomainId,
        key: &[u8],
        value: Option<&[u8]>,
    ) -> Result<(), JournalError> {
        self.configured_snapshot(domain)?;
        Self::validate_key(key)?;
        if let Some(value) = value {
            Self::validate_value(value)?;
        }

        let top = self.frames.last().ok_or(JournalError::AccountingInvariant)?;
        let existing = top
            .writes
            .get(&domain)
            .and_then(|domain_writes| domain_writes.get(key));
        let old_bytes = existing
            .map(|old| JournalFrame::entry_bytes(key.len(), old))
            .unwrap_or(0);
        let new_bytes = key
            .len()
            .checked_add(value.map_or(0, |bytes| bytes.len()))
            .ok_or(JournalError::ByteLimitExceeded)?;

        let next_live_entries = if existing.is_some() {
            self.live_entries
        } else {
            self.live_entries
                .checked_add(1)
                .ok_or(JournalError::EntryLimitExceeded)?
        };
        if next_live_entries > self.limits.max_entries {
            return Err(JournalError::EntryLimitExceeded);
        }

        let next_retained_bytes = self
            .retained_bytes
            .checked_sub(old_bytes)
            .and_then(|bytes| bytes.checked_add(new_bytes))
            .ok_or(JournalError::ByteLimitExceeded)?;
        if next_retained_bytes > self.limits.max_bytes {
            return Err(JournalError::ByteLimitExceeded);
        }

        let next_frame_entries = if existing.is_some() {
            top.entries
        } else {
            top.entries
                .checked_add(1)
                .ok_or(JournalError::AccountingInvariant)?
        };
        let next_frame_bytes = top
            .retained_bytes
            .checked_sub(old_bytes)
            .and_then(|bytes| bytes.checked_add(new_bytes))
            .ok_or(JournalError::AccountingInvariant)?;

        let owned_value = match value {
            Some(value) => JournalValue::Put(value.to_vec()),
            None => JournalValue::Delete,
        };
        let owned_key = key.to_vec();

        let top = self
            .frames
            .last_mut()
            .ok_or(JournalError::AccountingInvariant)?;
        top.writes
            .entry(domain)
            .or_default()
            .insert(owned_key, owned_value);
        top.entries = next_frame_entries;
        top.retained_bytes = next_frame_bytes;
        self.live_entries = next_live_entries;
        self.retained_bytes = next_retained_bytes;
        Ok(())
    }
}
