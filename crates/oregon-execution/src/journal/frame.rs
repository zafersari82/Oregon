use std::collections::BTreeMap;

use oregon_primitives::state_commitment::CommitmentDomainId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum JournalValue {
    Put(Vec<u8>),
    Delete,
}

impl JournalValue {
    pub(super) fn value_len(&self) -> usize {
        match self {
            Self::Put(value) => value.len(),
            Self::Delete => 0,
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct JournalFrame {
    pub(super) writes: BTreeMap<CommitmentDomainId, BTreeMap<Vec<u8>, JournalValue>>,
    pub(super) entries: usize,
    pub(super) retained_bytes: usize,
}

impl JournalFrame {
    pub(super) fn entry_bytes(key_len: usize, value: &JournalValue) -> usize {
        key_len + value.value_len()
    }
}
