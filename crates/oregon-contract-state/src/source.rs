use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;

use crate::{SMT_DEPTH, StateError, StateNode, value_hash};

pub trait StateSource {
    fn get_node(&self, node_hash: &Hash256) -> Result<Option<StateNode>, StateError>;
    fn get_value(&self, value_hash: &Hash256) -> Result<Option<Vec<u8>>, StateError>;
}

pub(crate) fn load_checked_node<S: StateSource + ?Sized>(
    source: &S,
    domain: CommitmentDomainId,
    requested_hash: Hash256,
    depth: usize,
) -> Result<StateNode, StateError> {
    let node = source
        .get_node(&requested_hash)?
        .ok_or(StateError::MissingNode(requested_hash))?;
    if node.hash(domain)? != requested_hash {
        return Err(StateError::NodeHashMismatch(requested_hash));
    }

    match &node {
        StateNode::Branch { depth: actual, .. } if depth < SMT_DEPTH => {
            if *actual as usize != depth {
                return Err(StateError::NodeDepthMismatch {
                    expected: depth as u16,
                    actual: *actual,
                });
            }
            Ok(node)
        }
        StateNode::Leaf { .. } if depth < SMT_DEPTH => Err(StateError::UnexpectedLeaf),
        StateNode::Branch { .. } => Err(StateError::UnexpectedBranch),
        StateNode::Leaf { .. } => Ok(node),
    }
}

pub(crate) fn load_checked_value<S: StateSource + ?Sized>(
    source: &S,
    domain: CommitmentDomainId,
    committed_value_hash: Hash256,
) -> Result<Vec<u8>, StateError> {
    let value = source
        .get_value(&committed_value_hash)?
        .ok_or(StateError::MissingValue(committed_value_hash))?;
    if value_hash(domain, &value)? != committed_value_hash {
        return Err(StateError::ValueHashMismatch(committed_value_hash));
    }
    Ok(value)
}
