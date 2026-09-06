use oregon_contract_state::{StateSource, read_value};
use oregon_primitives::state_commitment::CommitmentDomainId;

use super::ExecutionJournalV1;
use super::frame::JournalValue;
use super::types::JournalError;

impl<S: StateSource + ?Sized> ExecutionJournalV1<'_, S> {
    pub fn read(
        &self,
        domain: CommitmentDomainId,
        key: &[u8],
    ) -> Result<Option<Vec<u8>>, JournalError> {
        let snapshot = self.configured_snapshot(domain)?;
        Self::validate_key(key)?;

        for frame in self.frames.iter().rev() {
            let Some(domain_writes) = frame.writes.get(&domain) else {
                continue;
            };
            let Some(value) = domain_writes.get(key) else {
                continue;
            };
            return Ok(match value {
                JournalValue::Put(bytes) => Some(bytes.clone()),
                JournalValue::Delete => None,
            });
        }

        Ok(read_value(self.source, snapshot, key)?)
    }

    pub fn put(
        &mut self,
        domain: CommitmentDomainId,
        key: &[u8],
        value: &[u8],
    ) -> Result<(), JournalError> {
        self.stage_write(domain, key, Some(value))
    }

    pub fn delete(&mut self, domain: CommitmentDomainId, key: &[u8]) -> Result<(), JournalError> {
        self.stage_write(domain, key, None)
    }
}
