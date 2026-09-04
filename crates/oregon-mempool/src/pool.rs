use std::collections::{BTreeMap, HashMap};

use oregon_primitives::{Hash256, OutPoint, Transaction};
use oregon_utxo::{SpendVerifier, UtxoState};

use crate::admission::{commit_admission, preflight_admission_plan, prepare_admission};
use crate::capacity::plan_capacity;
use crate::graph::topological_order;
use crate::{AdmissionOutcome, MempoolConfig, MempoolEntry, MempoolError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainBase {
    pub tip_id: Hash256,
    pub tip_height: u64,
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
        let mut plan = prepare_admission(self, transaction, chain_base, chain_utxos, verifier)?;
        let new_total_bytes = plan_capacity(self, &mut plan)?;
        preflight_admission_plan(self, &plan, new_total_bytes)?;
        Ok(commit_admission(self, plan, new_total_bytes))
    }
}
