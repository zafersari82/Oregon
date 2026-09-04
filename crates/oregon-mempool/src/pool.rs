use std::collections::{BTreeMap, HashMap};

use oregon_primitives::{Hash256, OutPoint};

use crate::{MempoolConfig, MempoolEntry, MempoolError};

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
        Ok(self.entries.keys().copied().collect())
    }
}
