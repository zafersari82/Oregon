use std::collections::BTreeMap;

use oregon_contract_state::{StateError, StateNode, StateSource, StateTransition};
use oregon_primitives::Hash256;

#[derive(Default)]
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
