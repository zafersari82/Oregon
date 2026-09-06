use oregon_contract_state::{StateSource, StateWrite, StateWriteSet, apply_write_set};

use super::ExecutionJournalV1;
use super::frame::JournalValue;
use super::types::{JournalDomainRootsV1, JournalError, JournalIntentV1, JournalResultV1};

impl<S: StateSource + ?Sized> ExecutionJournalV1<'_, S> {
    pub fn finalize(self, intent: JournalIntentV1) -> Result<JournalResultV1, JournalError> {
        if self.frames.len() != 1 {
            return Err(JournalError::OpenChildFrames);
        }

        let Self {
            source,
            context,
            snapshots,
            frames,
            ..
        } = self;

        if intent == JournalIntentV1::Reverted {
            let roots = snapshots
                .into_values()
                .map(|snapshot| JournalDomainRootsV1 {
                    domain: snapshot.domain,
                    old_root: snapshot.root,
                    new_root: snapshot.root,
                })
                .collect();
            return Ok(JournalResultV1 {
                context,
                roots,
                transitions: Vec::new(),
            });
        }

        let root_frame = frames
            .into_iter()
            .next()
            .ok_or(JournalError::AccountingInvariant)?;
        let mut final_writes = root_frame.writes;
        let mut roots = Vec::with_capacity(snapshots.len());
        let mut transitions = Vec::new();

        for (domain, snapshot) in snapshots {
            let Some(domain_writes) = final_writes.remove(&domain) else {
                roots.push(JournalDomainRootsV1 {
                    domain,
                    old_root: snapshot.root,
                    new_root: snapshot.root,
                });
                continue;
            };

            let writes = domain_writes
                .into_iter()
                .map(|(key, value)| match value {
                    JournalValue::Put(bytes) => StateWrite::put(key, bytes),
                    JournalValue::Delete => StateWrite::delete(key),
                })
                .collect();
            let write_set = StateWriteSet::new(domain, writes)?;
            let transition = apply_write_set(source, snapshot, &write_set)?;
            roots.push(JournalDomainRootsV1 {
                domain,
                old_root: snapshot.root,
                new_root: transition.new_root,
            });
            if transition.new_root != snapshot.root {
                transitions.push(transition);
            }
        }

        if !final_writes.is_empty() {
            return Err(JournalError::AccountingInvariant);
        }

        Ok(JournalResultV1 {
            context,
            roots,
            transitions,
        })
    }
}
