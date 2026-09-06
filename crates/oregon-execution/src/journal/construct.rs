use std::collections::BTreeMap;

use oregon_contract_state::{
    DomainSnapshot, MAX_STATE_KEY_BYTES, MAX_STATE_VALUE_BYTES, StateError, StateSource,
};
use oregon_primitives::state_commitment::CommitmentDomainId;

use super::ExecutionJournalV1;
use super::frame::JournalFrame;
use super::types::{JournalContextV1, JournalError, JournalLimitsV1, MAX_JOURNAL_SNAPSHOTS};

impl<'a, S: StateSource + ?Sized> ExecutionJournalV1<'a, S> {
    pub fn new(
        source: &'a S,
        context: JournalContextV1,
        snapshots: &[DomainSnapshot],
        limits: JournalLimitsV1,
    ) -> Result<Self, JournalError> {
        if snapshots.is_empty() {
            return Err(JournalError::EmptySnapshots);
        }
        if snapshots.len() > MAX_JOURNAL_SNAPSHOTS {
            return Err(JournalError::TooManySnapshots(snapshots.len()));
        }

        let mut configured = BTreeMap::new();
        for snapshot in snapshots {
            Self::validate_supported_domain(snapshot.domain)?;
            if configured.insert(snapshot.domain, *snapshot).is_some() {
                return Err(JournalError::DuplicateSnapshotDomain(snapshot.domain));
            }
        }

        Ok(Self {
            source,
            context,
            snapshots: configured,
            frames: vec![JournalFrame::default()],
            limits,
            total_frames_created: 1,
            live_entries: 0,
            retained_bytes: 0,
        })
    }

    pub(super) fn configured_snapshot(
        &self,
        domain: CommitmentDomainId,
    ) -> Result<DomainSnapshot, JournalError> {
        Self::validate_supported_domain(domain)?;
        self.snapshots
            .get(&domain)
            .copied()
            .ok_or(JournalError::UnconfiguredDomain(domain))
    }

    fn validate_supported_domain(domain: CommitmentDomainId) -> Result<(), JournalError> {
        match domain {
            CommitmentDomainId::Wasm
            | CommitmentDomainId::ExecutionAccounting
            | CommitmentDomainId::ExecutionReceipts
            | CommitmentDomainId::AsyncOutbox
            | CommitmentDomainId::AsyncConsumed
            | CommitmentDomainId::FeeState => Ok(()),
            CommitmentDomainId::NativeUtxo | CommitmentDomainId::Evm => {
                Err(JournalError::UnsupportedDomain(domain))
            }
        }
    }

    pub(super) fn validate_key(key: &[u8]) -> Result<(), JournalError> {
        if key.len() > MAX_STATE_KEY_BYTES {
            return Err(StateError::KeyTooLarge(key.len()).into());
        }
        Ok(())
    }

    pub(super) fn validate_value(value: &[u8]) -> Result<(), JournalError> {
        if value.len() > MAX_STATE_VALUE_BYTES {
            return Err(StateError::ValueTooLarge(value.len()).into());
        }
        Ok(())
    }
}
