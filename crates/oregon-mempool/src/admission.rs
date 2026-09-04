use std::collections::{BTreeSet, HashSet};

use oregon_consensus::validate_normal_transaction_skeleton;
use oregon_primitives::{Hash256, OutPoint, Transaction};
use oregon_utxo::{SpendVerifier, UtxoState};

use crate::capacity::removed_bytes;
use crate::graph::{ancestor_closure, descendant_closure, topological_order};
use crate::pool::{ChainBase, Mempool};
use crate::{AdmissionOutcome, MempoolEntry, MempoolError};

pub(crate) struct PreparedCandidate {
    pub(crate) entry: MempoolEntry,
    pub(crate) spend_claims: Vec<OutPoint>,
    pub(crate) ancestors: BTreeSet<Hash256>,
}

pub(crate) struct AdmissionPlan {
    pub(crate) candidate: PreparedCandidate,
    pub(crate) remove: BTreeSet<Hash256>,
}

pub(crate) fn prepare_admission<V: SpendVerifier>(
    pool: &Mempool,
    transaction: Transaction,
    chain_base: ChainBase,
    chain_utxos: &UtxoState,
    verifier: &V,
) -> Result<AdmissionPlan, MempoolError> {
    if chain_base != pool.base {
        return Err(MempoolError::StaleChainContext);
    }

    let spend_height = chain_base
        .tip_height
        .checked_add(1)
        .ok_or(MempoolError::HeightOverflow)?;

    let encoded_bytes = transaction.encode().len();
    let txid = transaction.txid();
    validate_normal_transaction_skeleton(&transaction)?;

    if pool.entries.contains_key(&txid) {
        return Err(MempoolError::AlreadyKnown(txid));
    }

    let spend_claims: Vec<_> = transaction
        .inputs
        .iter()
        .map(|input| input.outpoint())
        .collect();
    for outpoint in &spend_claims {
        if let Some(existing_txid) = pool.spenders.get(outpoint) {
            return Err(MempoolError::Conflict {
                outpoint: *outpoint,
                existing_txid: *existing_txid,
            });
        }
    }

    let mut direct_parents = BTreeSet::new();
    for outpoint in &spend_claims {
        if chain_utxos.get(outpoint).is_some() {
            continue;
        }

        let Some(parent) = pool.entries.get(&outpoint.txid) else {
            return Err(MempoolError::MissingDependency(*outpoint));
        };
        if parent
            .transaction
            .outputs
            .get(outpoint.index as usize)
            .is_none()
        {
            return Err(MempoolError::InvalidParentOutput(*outpoint));
        }
        direct_parents.insert(parent.txid);
    }

    let ancestors = ancestor_closure(&pool.entries, &direct_parents)?;
    if ancestors.len() > pool.config.max_ancestors {
        return Err(MempoolError::TooManyAncestors);
    }
    for ancestor in &ancestors {
        let descendants = descendant_closure(&pool.entries, *ancestor)?;
        let with_candidate = descendants
            .len()
            .checked_add(1)
            .ok_or(MempoolError::InvariantViolation)?;
        if with_candidate > pool.config.max_descendants {
            return Err(MempoolError::TooManyDescendants);
        }
    }

    let full_order = topological_order(&pool.entries)?;
    let replay_order: Vec<_> = full_order
        .into_iter()
        .filter(|ancestor| ancestors.contains(ancestor))
        .collect();
    if replay_order.len() != ancestors.len() {
        return Err(MempoolError::InvariantViolation);
    }

    let mut seeded = HashSet::new();
    let mut narrow_entries = Vec::new();
    for ancestor in &replay_order {
        let entry = pool
            .entries
            .get(ancestor)
            .ok_or(MempoolError::InvariantViolation)?;
        seed_chain_inputs(
            &entry.transaction,
            chain_utxos,
            &mut seeded,
            &mut narrow_entries,
        );
    }
    seed_chain_inputs(&transaction, chain_utxos, &mut seeded, &mut narrow_entries);

    let mut replay_txids = ancestors.clone();
    replay_txids.insert(txid);
    for (outpoint, entry) in chain_utxos.entries() {
        if replay_txids.contains(&outpoint.txid) && seeded.insert(*outpoint) {
            narrow_entries.push((*outpoint, entry.clone()));
        }
    }

    let mut validation_state = UtxoState::try_from_entries(narrow_entries)?;
    for ancestor in &replay_order {
        let entry = pool
            .entries
            .get(ancestor)
            .ok_or(MempoolError::InvariantViolation)?;
        let replayed_fee = validation_state.apply_normal_transaction(
            &entry.transaction,
            spend_height,
            verifier,
        )?;
        if replayed_fee != entry.fee {
            return Err(MempoolError::InvariantViolation);
        }
    }
    let fee = validation_state.apply_normal_transaction(&transaction, spend_height, verifier)?;

    Ok(AdmissionPlan {
        candidate: PreparedCandidate {
            entry: MempoolEntry {
                transaction,
                txid,
                fee,
                encoded_bytes,
                parents: direct_parents,
                children: BTreeSet::new(),
            },
            spend_claims,
            ancestors,
        },
        remove: BTreeSet::new(),
    })
}

pub(crate) fn preflight_admission_plan(
    pool: &Mempool,
    plan: &AdmissionPlan,
    new_total_bytes: usize,
) -> Result<(), MempoolError> {
    topological_order(&pool.entries)?;

    if plan
        .remove
        .iter()
        .any(|txid| plan.candidate.ancestors.contains(txid))
    {
        return Err(MempoolError::InvariantViolation);
    }

    for parent in &plan.candidate.entry.parents {
        if !plan.candidate.ancestors.contains(parent)
            || plan.remove.contains(parent)
            || !pool.entries.contains_key(parent)
        {
            return Err(MempoolError::InvariantViolation);
        }
    }
    for ancestor in &plan.candidate.ancestors {
        if plan.remove.contains(ancestor) || !pool.entries.contains_key(ancestor) {
            return Err(MempoolError::InvariantViolation);
        }
    }
    for outpoint in &plan.candidate.spend_claims {
        if pool.spenders.contains_key(outpoint) {
            return Err(MempoolError::InvariantViolation);
        }
    }

    for txid in &plan.remove {
        let entry = pool
            .entries
            .get(txid)
            .ok_or(MempoolError::InvariantViolation)?;
        for input in &entry.transaction.inputs {
            if pool.spenders.get(&input.outpoint()) != Some(txid) {
                return Err(MempoolError::InvariantViolation);
            }
        }
        if entry
            .children
            .iter()
            .any(|child| !plan.remove.contains(child))
        {
            return Err(MempoolError::InvariantViolation);
        }
    }

    let removed_bytes = removed_bytes(pool, &plan.remove)?;
    let expected_bytes = pool
        .total_bytes
        .checked_sub(removed_bytes)
        .and_then(|bytes| bytes.checked_add(plan.candidate.entry.encoded_bytes))
        .ok_or(MempoolError::InvariantViolation)?;
    if expected_bytes != new_total_bytes || expected_bytes > pool.config.max_total_bytes {
        return Err(MempoolError::InvariantViolation);
    }

    let expected_entries = pool
        .entries
        .len()
        .checked_sub(plan.remove.len())
        .and_then(|count| count.checked_add(1))
        .ok_or(MempoolError::InvariantViolation)?;
    if expected_entries > pool.config.max_entries {
        return Err(MempoolError::InvariantViolation);
    }

    Ok(())
}

pub(crate) fn commit_admission(
    pool: &mut Mempool,
    plan: AdmissionPlan,
    new_total_bytes: usize,
) -> AdmissionOutcome {
    let AdmissionPlan { candidate, remove } = plan;
    let txid = candidate.entry.txid;
    let fee = candidate.entry.fee;
    let encoded_bytes = candidate.entry.encoded_bytes;
    let parents = candidate.entry.parents.clone();
    let evicted: Vec<_> = remove.iter().copied().collect();

    let removal_commit: Vec<_> = remove
        .iter()
        .map(|remove_txid| {
            let entry = pool
                .entries
                .get(remove_txid)
                .expect("removal entry was preflighted");
            let spend_claims = entry
                .transaction
                .inputs
                .iter()
                .map(|input| input.outpoint())
                .collect::<Vec<_>>();
            let surviving_parents = entry
                .parents
                .iter()
                .filter(|parent| !remove.contains(parent))
                .copied()
                .collect::<Vec<_>>();
            (*remove_txid, spend_claims, surviving_parents)
        })
        .collect();

    for (remove_txid, spend_claims, surviving_parents) in removal_commit {
        for parent in surviving_parents {
            let parent_entry = pool
                .entries
                .get_mut(&parent)
                .expect("surviving parent was preflighted");
            let removed = parent_entry.children.remove(&remove_txid);
            debug_assert!(removed);
        }
        for outpoint in spend_claims {
            let removed = pool.spenders.remove(&outpoint);
            debug_assert_eq!(removed, Some(remove_txid));
        }
        let removed = pool.entries.remove(&remove_txid);
        debug_assert!(removed.is_some());
    }

    for parent in &parents {
        let parent_entry = pool
            .entries
            .get_mut(parent)
            .expect("candidate parent was preflighted");
        let inserted = parent_entry.children.insert(txid);
        debug_assert!(inserted);
    }
    for outpoint in &candidate.spend_claims {
        let previous = pool.spenders.insert(*outpoint, txid);
        debug_assert!(previous.is_none());
    }
    let previous = pool.entries.insert(txid, candidate.entry);
    debug_assert!(previous.is_none());
    pool.total_bytes = new_total_bytes;

    AdmissionOutcome {
        txid,
        fee,
        encoded_bytes,
        evicted,
    }
}

fn seed_chain_inputs(
    transaction: &Transaction,
    chain_utxos: &UtxoState,
    seeded: &mut HashSet<OutPoint>,
    narrow_entries: &mut Vec<(OutPoint, oregon_utxo::UtxoEntry)>,
) {
    for input in &transaction.inputs {
        let outpoint = input.outpoint();
        if let Some(entry) = chain_utxos.get(&outpoint) {
            if seeded.insert(outpoint) {
                narrow_entries.push((outpoint, entry.clone()));
            }
        }
    }
}
