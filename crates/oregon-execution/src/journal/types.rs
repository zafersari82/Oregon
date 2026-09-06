use oregon_contract_state::{StateError, StateTransition};
use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;
use thiserror::Error;

pub(super) const MAX_JOURNAL_DEPTH: usize = 64;
pub(super) const MAX_JOURNAL_FRAMES: usize = 4_096;
pub(super) const MAX_JOURNAL_ENTRIES: usize = 65_536;
pub(super) const MAX_JOURNAL_BYTES: usize = 16_777_216;
pub(super) const MAX_JOURNAL_SNAPSHOTS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JournalContextV1 {
    pub chain_id: u64,
    pub height: u64,
    pub parent_block_hash: Hash256,
    pub txid: Hash256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JournalLimitsV1 {
    pub(super) max_depth: usize,
    pub(super) max_frames: usize,
    pub(super) max_entries: usize,
    pub(super) max_bytes: usize,
}

impl JournalLimitsV1 {
    pub fn new(
        max_depth: usize,
        max_frames: usize,
        max_entries: usize,
        max_bytes: usize,
    ) -> Result<Self, JournalError> {
        if max_depth == 0
            || max_frames == 0
            || max_entries == 0
            || max_bytes == 0
            || max_depth > MAX_JOURNAL_DEPTH
            || max_frames > MAX_JOURNAL_FRAMES
            || max_entries > MAX_JOURNAL_ENTRIES
            || max_bytes > MAX_JOURNAL_BYTES
        {
            return Err(JournalError::InvalidLimits);
        }
        Ok(Self {
            max_depth,
            max_frames,
            max_entries,
            max_bytes,
        })
    }
}

impl Default for JournalLimitsV1 {
    fn default() -> Self {
        Self {
            max_depth: MAX_JOURNAL_DEPTH,
            max_frames: MAX_JOURNAL_FRAMES,
            max_entries: MAX_JOURNAL_ENTRIES,
            max_bytes: MAX_JOURNAL_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalIntentV1 {
    Committed,
    Reverted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JournalDomainRootsV1 {
    pub domain: CommitmentDomainId,
    pub old_root: Hash256,
    pub new_root: Hash256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalResultV1 {
    pub context: JournalContextV1,
    pub roots: Vec<JournalDomainRootsV1>,
    pub transitions: Vec<StateTransition>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum JournalError {
    #[error("journal limits must be positive and within Stage 4A structural ceilings")]
    InvalidLimits,
    #[error("journal snapshot set is empty")]
    EmptySnapshots,
    #[error("journal snapshot set has {0} entries, exceeding the Stage 4A limit")]
    TooManySnapshots(usize),
    #[error("duplicate journal snapshot domain {0:?}")]
    DuplicateSnapshotDomain(CommitmentDomainId),
    #[error("unsupported journal state domain {0:?}")]
    UnsupportedDomain(CommitmentDomainId),
    #[error("journal state domain {0:?} was not configured at construction")]
    UnconfiguredDomain(CommitmentDomainId),
    #[error("the root frame cannot be ended through child frame lifecycle operations")]
    RootFrameLifecycle,
    #[error("journal finalization requires exactly the root frame to remain open")]
    OpenChildFrames,
    #[error("journal live frame depth limit exceeded")]
    DepthLimitExceeded,
    #[error("journal total frame creation limit exceeded")]
    FrameLimitExceeded,
    #[error("journal live write-entry limit exceeded")]
    EntryLimitExceeded,
    #[error("journal retained raw-byte limit exceeded")]
    ByteLimitExceeded,
    #[error("journal resource accounting invariant violated")]
    AccountingInvariant,
    #[error(transparent)]
    State(#[from] StateError),
}
