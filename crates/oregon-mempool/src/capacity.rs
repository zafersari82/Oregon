use std::collections::BTreeSet;

use oregon_primitives::Hash256;

use crate::MempoolError;
use crate::admission::AdmissionPlan;
use crate::eviction::eviction_cmp;
use crate::graph::descendant_closure;
use crate::pool::Mempool;

pub(crate) fn plan_capacity(
    pool: &Mempool,
    plan: &mut AdmissionPlan,
) -> Result<usize, MempoolError> {
    loop {
        let removed_bytes = removed_bytes(pool, &plan.remove)?;
        let virtual_entries = pool
            .entries
            .len()
            .checked_add(1)
            .and_then(|count| count.checked_sub(plan.remove.len()))
            .ok_or(MempoolError::InvariantViolation)?;
        let virtual_bytes = pool
            .total_bytes
            .checked_add(plan.candidate.entry.encoded_bytes)
            .and_then(|bytes| bytes.checked_sub(removed_bytes))
            .ok_or(MempoolError::InvariantViolation)?;

        if virtual_entries <= pool.config.max_entries
            && virtual_bytes <= pool.config.max_total_bytes
        {
            return Ok(virtual_bytes);
        }

        let mut selected_txid = None;
        let mut selected_entry = &plan.candidate.entry;
        for (txid, entry) in &pool.entries {
            if plan.remove.contains(txid) {
                continue;
            }
            if eviction_cmp(entry, selected_entry).is_lt() {
                selected_txid = Some(*txid);
                selected_entry = entry;
            }
        }

        let Some(root) = selected_txid else {
            return Err(MempoolError::CapacityRejected);
        };
        if plan.candidate.ancestors.contains(&root) {
            return Err(MempoolError::CapacityRejected);
        }

        let descendants = descendant_closure(&pool.entries, root)?;
        if descendants
            .iter()
            .any(|txid| plan.candidate.ancestors.contains(txid))
        {
            return Err(MempoolError::CapacityRejected);
        }

        let inserted = plan.remove.insert(root);
        if !inserted {
            return Err(MempoolError::InvariantViolation);
        }
        plan.remove.extend(descendants);
    }
}

pub(crate) fn removed_bytes(
    pool: &Mempool,
    remove: &BTreeSet<Hash256>,
) -> Result<usize, MempoolError> {
    let mut removed_bytes = 0usize;
    for txid in remove {
        let entry = pool
            .entries
            .get(txid)
            .ok_or(MempoolError::InvariantViolation)?;
        removed_bytes = removed_bytes
            .checked_add(entry.encoded_bytes)
            .ok_or(MempoolError::InvariantViolation)?;
    }
    Ok(removed_bytes)
}
