use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use oregon_consensus::validate_normal_transaction_skeleton;
use oregon_primitives::{Hash256, OutPoint, Transaction};
use oregon_utxo::{SpendVerifier, UtxoState};

use crate::eviction::eviction_cmp;
use crate::graph::{ancestor_closure, descendant_closure, topological_order};
use crate::{AdmissionOutcome, MempoolConfig, MempoolEntry, MempoolError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainBase {
    pub tip_id: Hash256,
    pub tip_height: u64,
}

struct PreparedCandidate {
    entry: MempoolEntry,
    spend_claims: Vec<OutPoint>,
    ancestors: BTreeSet<Hash256>,
}

struct AdmissionPlan {
    candidate: PreparedCandidate,
    remove: BTreeSet<Hash256>,
}

pub struct Mempool {
    pub(crate) config: MempoolConfig,
    pub(crate) base: ChainBase,
    pub(crate) entries: BTreeMap<Hash256, MempoolEntry>,
    pub(crate) spenders: HashMap<OutPoint, Hash256>,
    pub(crate) total_bytes: usize,
}

impl Mempool {
    pub fn new(base: ChainBase, config: MempoolConfig) -> Result<Self, MempoolError> {
        if config.max_entries == 0 || config.max_total_bytes == 0 {
            return Err(MempoolError::InvalidConfig);
        }

        Ok(Self {
            config,
            base,
            entries: BTreeMap::new(),
            spenders: HashMap::new(),
            total_bytes: 0,
        })
    }

    pub fn base(&self) -> ChainBase {
        self.base
    }

    pub fn len(&self) -> usize {
        debug_assert!(self.entries.len() <= self.config.max_entries);
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        debug_assert_eq!(self.entries.is_empty(), self.spenders.is_empty());
        self.entries.is_empty()
    }

    pub fn total_bytes(&self) -> usize {
        debug_assert!(self.total_bytes <= self.config.max_total_bytes);
        self.total_bytes
    }

    pub fn contains(&self, txid: &Hash256) -> bool {
        self.entries.contains_key(txid)
    }

    pub fn entry(&self, txid: &Hash256) -> Option<&MempoolEntry> {
        self.entries.get(txid)
    }

    pub fn deterministic_order(&self) -> Result<Vec<Hash256>, MempoolError> {
        topological_order(&self.entries)
    }

    pub fn admit<V: SpendVerifier>(
        &mut self,
        transaction: Transaction,
        chain_base: ChainBase,
        chain_utxos: &UtxoState,
        verifier: &V,
    ) -> Result<AdmissionOutcome, MempoolError> {
        let (mut plan, _) =
            self.prepare_admission(transaction, chain_base, chain_utxos, verifier)?;
        let new_total_bytes = self.plan_capacity(&mut plan)?;
        self.preflight_admission_plan(&plan, new_total_bytes)?;

        let txid = plan.candidate.entry.txid;
        let fee = plan.candidate.entry.fee;
        let encoded_bytes = plan.candidate.entry.encoded_bytes;
        let parents = plan.candidate.entry.parents.clone();
        let evicted: Vec<_> = plan.remove.iter().copied().collect();

        let removal_commit: Vec<_> = plan
            .remove
            .iter()
            .map(|remove_txid| {
                let entry = self
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
                    .filter(|parent| !plan.remove.contains(parent))
                    .copied()
                    .collect::<Vec<_>>();
                (*remove_txid, spend_claims, surviving_parents)
            })
            .collect();

        for (remove_txid, spend_claims, surviving_parents) in removal_commit {
            for parent in surviving_parents {
                let parent_entry = self
                    .entries
                    .get_mut(&parent)
                    .expect("surviving parent was preflighted");
                let removed = parent_entry.children.remove(&remove_txid);
                debug_assert!(removed);
            }
            for outpoint in spend_claims {
                let removed = self.spenders.remove(&outpoint);
                debug_assert_eq!(removed, Some(remove_txid));
            }
            let removed = self.entries.remove(&remove_txid);
            debug_assert!(removed.is_some());
        }

        for parent in &parents {
            let parent_entry = self
                .entries
                .get_mut(parent)
                .expect("candidate parent was preflighted");
            let inserted = parent_entry.children.insert(txid);
            debug_assert!(inserted);
        }
        for outpoint in &plan.candidate.spend_claims {
            let previous = self.spenders.insert(*outpoint, txid);
            debug_assert!(previous.is_none());
        }
        let previous = self.entries.insert(txid, plan.candidate.entry);
        debug_assert!(previous.is_none());
        self.total_bytes = new_total_bytes;

        Ok(AdmissionOutcome {
            txid,
            fee,
            encoded_bytes,
            evicted,
        })
    }

    fn plan_capacity(&self, plan: &mut AdmissionPlan) -> Result<usize, MempoolError> {
        loop {
            let removed_bytes = self.removed_bytes(&plan.remove)?;
            let virtual_entries = self
                .entries
                .len()
                .checked_add(1)
                .and_then(|count| count.checked_sub(plan.remove.len()))
                .ok_or(MempoolError::InvariantViolation)?;
            let virtual_bytes = self
                .total_bytes
                .checked_add(plan.candidate.entry.encoded_bytes)
                .and_then(|bytes| bytes.checked_sub(removed_bytes))
                .ok_or(MempoolError::InvariantViolation)?;

            if virtual_entries <= self.config.max_entries
                && virtual_bytes <= self.config.max_total_bytes
            {
                return Ok(virtual_bytes);
            }

            let mut selected_txid = None;
            let mut selected_entry = &plan.candidate.entry;
            for (txid, entry) in &self.entries {
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

            let descendants = descendant_closure(&self.entries, root)?;
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

    fn removed_bytes(&self, remove: &BTreeSet<Hash256>) -> Result<usize, MempoolError> {
        let mut removed_bytes = 0usize;
        for txid in remove {
            let entry = self
                .entries
                .get(txid)
                .ok_or(MempoolError::InvariantViolation)?;
            removed_bytes = removed_bytes
                .checked_add(entry.encoded_bytes)
                .ok_or(MempoolError::InvariantViolation)?;
        }
        Ok(removed_bytes)
    }

    fn preflight_admission_plan(
        &self,
        plan: &AdmissionPlan,
        new_total_bytes: usize,
    ) -> Result<(), MempoolError> {
        topological_order(&self.entries)?;

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
                || !self.entries.contains_key(parent)
            {
                return Err(MempoolError::InvariantViolation);
            }
        }
        for ancestor in &plan.candidate.ancestors {
            if plan.remove.contains(ancestor) || !self.entries.contains_key(ancestor) {
                return Err(MempoolError::InvariantViolation);
            }
        }
        for outpoint in &plan.candidate.spend_claims {
            if self.spenders.contains_key(outpoint) {
                return Err(MempoolError::InvariantViolation);
            }
        }

        for txid in &plan.remove {
            let entry = self
                .entries
                .get(txid)
                .ok_or(MempoolError::InvariantViolation)?;
            for input in &entry.transaction.inputs {
                if self.spenders.get(&input.outpoint()) != Some(txid) {
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

        let removed_bytes = self.removed_bytes(&plan.remove)?;
        let expected_bytes = self
            .total_bytes
            .checked_sub(removed_bytes)
            .and_then(|bytes| bytes.checked_add(plan.candidate.entry.encoded_bytes))
            .ok_or(MempoolError::InvariantViolation)?;
        if expected_bytes != new_total_bytes || expected_bytes > self.config.max_total_bytes {
            return Err(MempoolError::InvariantViolation);
        }

        let expected_entries = self
            .entries
            .len()
            .checked_sub(plan.remove.len())
            .and_then(|count| count.checked_add(1))
            .ok_or(MempoolError::InvariantViolation)?;
        if expected_entries > self.config.max_entries {
            return Err(MempoolError::InvariantViolation);
        }

        Ok(())
    }

    fn prepare_admission<V: SpendVerifier>(
        &self,
        transaction: Transaction,
        chain_base: ChainBase,
        chain_utxos: &UtxoState,
        verifier: &V,
    ) -> Result<(AdmissionPlan, usize), MempoolError> {
        if chain_base != self.base {
            return Err(MempoolError::StaleChainContext);
        }

        let spend_height = chain_base
            .tip_height
            .checked_add(1)
            .ok_or(MempoolError::HeightOverflow)?;

        let encoded_bytes = transaction.encode().len();
        let txid = transaction.txid();
        validate_normal_transaction_skeleton(&transaction)?;

        if self.entries.contains_key(&txid) {
            return Err(MempoolError::AlreadyKnown(txid));
        }

        let spend_claims: Vec<_> = transaction
            .inputs
            .iter()
            .map(|input| input.outpoint())
            .collect();
        for outpoint in &spend_claims {
            if let Some(existing_txid) = self.spenders.get(outpoint) {
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

            let Some(parent) = self.entries.get(&outpoint.txid) else {
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

        let ancestors = ancestor_closure(&self.entries, &direct_parents)?;
        if ancestors.len() > self.config.max_ancestors {
            return Err(MempoolError::TooManyAncestors);
        }
        for ancestor in &ancestors {
            let descendants = descendant_closure(&self.entries, *ancestor)?;
            let with_candidate = descendants
                .len()
                .checked_add(1)
                .ok_or(MempoolError::InvariantViolation)?;
            if with_candidate > self.config.max_descendants {
                return Err(MempoolError::TooManyDescendants);
            }
        }

        let full_order = topological_order(&self.entries)?;
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
            let entry = self
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

        let mut validation_state = UtxoState::from_persisted_entries(narrow_entries)?;
        for ancestor in &replay_order {
            let entry = self
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
        let fee =
            validation_state.apply_normal_transaction(&transaction, spend_height, verifier)?;

        let new_total_bytes = self
            .total_bytes
            .checked_add(encoded_bytes)
            .ok_or(MempoolError::InvariantViolation)?;

        let candidate = PreparedCandidate {
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
        };

        Ok((
            AdmissionPlan {
                candidate,
                remove: BTreeSet::new(),
            },
            new_total_bytes,
        ))
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
