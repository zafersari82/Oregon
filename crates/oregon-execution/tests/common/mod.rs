use std::collections::BTreeMap;

use oregon_contract_state::{
    DomainSnapshot, StateError, StateNode, StateSource, StateTransition, StateWrite, StateWriteSet,
    apply_write_set, empty_hashes,
};
use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemorySource {
    pub nodes: BTreeMap<Hash256, StateNode>,
    pub values: BTreeMap<Hash256, Vec<u8>>,
}

impl StateSource for MemorySource {
    fn get_node(&self, node_hash: &Hash256) -> Result<Option<StateNode>, StateError> {
        Ok(self.nodes.get(node_hash).cloned())
    }

    fn get_value(&self, value_hash: &Hash256) -> Result<Option<Vec<u8>>, StateError> {
        Ok(self.values.get(value_hash).cloned())
    }
}

impl MemorySource {
    pub fn absorb(&mut self, transition: &StateTransition) {
        self.nodes.extend(
            transition
                .nodes
                .iter()
                .map(|(hash, node)| (*hash, node.clone())),
        );
        self.values.extend(
            transition
                .values
                .iter()
                .map(|(hash, value)| (*hash, value.clone())),
        );
    }
}

pub fn empty_snapshot(domain: CommitmentDomainId) -> DomainSnapshot {
    DomainSnapshot {
        domain,
        root: empty_hashes(domain)[0],
    }
}

pub fn seed_snapshot(
    source: &mut MemorySource,
    domain: CommitmentDomainId,
    writes: Vec<StateWrite>,
) -> DomainSnapshot {
    let base = empty_snapshot(domain);
    let write_set = StateWriteSet::new(domain, writes).expect("fixture write set is valid");
    let transition =
        apply_write_set(source, base, &write_set).expect("fixture transition is valid");
    source.absorb(&transition);
    DomainSnapshot {
        domain,
        root: transition.new_root,
    }
}
