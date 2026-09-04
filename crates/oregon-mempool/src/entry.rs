use std::collections::BTreeSet;

use oregon_primitives::{Hash256, Transaction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MempoolEntry {
    pub(crate) transaction: Transaction,
    pub(crate) txid: Hash256,
    pub(crate) fee: u64,
    pub(crate) encoded_bytes: usize,
    pub(crate) parents: BTreeSet<Hash256>,
    pub(crate) children: BTreeSet<Hash256>,
}

impl MempoolEntry {
    pub fn transaction(&self) -> &Transaction {
        &self.transaction
    }

    pub fn txid(&self) -> Hash256 {
        self.txid
    }

    pub fn fee(&self) -> u64 {
        self.fee
    }

    pub fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }

    pub fn parents(&self) -> &BTreeSet<Hash256> {
        &self.parents
    }

    pub fn children(&self) -> &BTreeSet<Hash256> {
        &self.children
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionOutcome {
    pub txid: Hash256,
    pub fee: u64,
    pub encoded_bytes: usize,
    pub evicted: Vec<Hash256>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconcileReport {
    pub removed: Vec<Hash256>,
    pub retained: usize,
}
