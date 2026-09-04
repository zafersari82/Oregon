use std::collections::{BTreeMap, BTreeSet};

use oregon_primitives::Hash256;

use crate::{MempoolEntry, MempoolError};

pub(crate) fn ancestor_closure(
    entries: &BTreeMap<Hash256, MempoolEntry>,
    direct_parents: &BTreeSet<Hash256>,
) -> Result<BTreeSet<Hash256>, MempoolError> {
    let mut closure = BTreeSet::new();
    let mut pending: Vec<_> = direct_parents.iter().copied().collect();

    while let Some(txid) = pending.pop() {
        let entry = entries.get(&txid).ok_or(MempoolError::InvariantViolation)?;
        if !closure.insert(txid) {
            continue;
        }
        pending.extend(entry.parents.iter().copied());
    }

    let order = topological_order(entries)?;
    let emitted = order
        .iter()
        .filter(|txid| closure.contains(txid))
        .count();
    if emitted != closure.len() {
        return Err(MempoolError::DependencyCycle);
    }

    Ok(closure)
}

pub(crate) fn descendant_closure(
    entries: &BTreeMap<Hash256, MempoolEntry>,
    root: Hash256,
) -> Result<BTreeSet<Hash256>, MempoolError> {
    let root_entry = entries.get(&root).ok_or(MempoolError::InvariantViolation)?;
    let mut closure = BTreeSet::new();
    let mut pending: Vec<_> = root_entry.children.iter().copied().collect();

    while let Some(txid) = pending.pop() {
        let entry = entries.get(&txid).ok_or(MempoolError::InvariantViolation)?;
        if !closure.insert(txid) {
            continue;
        }
        pending.extend(entry.children.iter().copied());
    }

    if closure.contains(&root) {
        return Err(MempoolError::DependencyCycle);
    }

    Ok(closure)
}

pub(crate) fn topological_order(
    entries: &BTreeMap<Hash256, MempoolEntry>,
) -> Result<Vec<Hash256>, MempoolError> {
    let mut indegree = BTreeMap::new();

    for (txid, entry) in entries {
        for parent in &entry.parents {
            let parent_entry = entries.get(parent).ok_or(MempoolError::InvariantViolation)?;
            if !parent_entry.children.contains(txid) {
                return Err(MempoolError::InvariantViolation);
            }
        }
        for child in &entry.children {
            let child_entry = entries.get(child).ok_or(MempoolError::InvariantViolation)?;
            if !child_entry.parents.contains(txid) {
                return Err(MempoolError::InvariantViolation);
            }
        }
        indegree.insert(*txid, entry.parents.len());
    }

    let mut ready: BTreeSet<_> = indegree
        .iter()
        .filter_map(|(txid, degree)| (*degree == 0).then_some(*txid))
        .collect();
    let mut emitted = Vec::with_capacity(entries.len());

    while let Some(txid) = ready.pop_first() {
        emitted.push(txid);
        let entry = entries.get(&txid).ok_or(MempoolError::InvariantViolation)?;
        for child in &entry.children {
            let degree = indegree
                .get_mut(child)
                .ok_or(MempoolError::InvariantViolation)?;
            *degree = degree
                .checked_sub(1)
                .ok_or(MempoolError::InvariantViolation)?;
            if *degree == 0 {
                ready.insert(*child);
            }
        }
    }

    if emitted.len() != entries.len() {
        return Err(MempoolError::DependencyCycle);
    }

    Ok(emitted)
}
