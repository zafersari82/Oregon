mod commit;
mod construct;
mod finalize;
mod frame;
mod io;
mod lifecycle;
mod types;
mod write;

use std::collections::BTreeMap;

use frame::JournalFrame;
use oregon_contract_state::{DomainSnapshot, StateSource};
use oregon_primitives::state_commitment::CommitmentDomainId;

pub use types::{
    JournalContextV1, JournalDomainRootsV1, JournalError, JournalIntentV1, JournalLimitsV1,
    JournalResultV1,
};

pub struct ExecutionJournalV1<'a, S: StateSource + ?Sized> {
    source: &'a S,
    context: JournalContextV1,
    snapshots: BTreeMap<CommitmentDomainId, DomainSnapshot>,
    frames: Vec<JournalFrame>,
    limits: JournalLimitsV1,
    total_frames_created: usize,
    live_entries: usize,
    retained_bytes: usize,
}
