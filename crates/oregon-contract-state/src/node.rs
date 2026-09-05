use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;

use crate::{StateError, branch_hash, empty_hashes, leaf_hash};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateNode {
    Leaf {
        path_key: Hash256,
        value_hash: Hash256,
    },
    Branch {
        depth: u16,
        left: Hash256,
        right: Hash256,
    },
}

impl StateNode {
    pub fn hash(&self, domain: CommitmentDomainId) -> Result<Hash256, StateError> {
        match self {
            Self::Leaf {
                path_key,
                value_hash,
            } => Ok(leaf_hash(domain, *path_key, *value_hash)),
            Self::Branch { depth, left, right } => {
                if *depth as usize >= 256 {
                    return Err(StateError::DepthOutOfRange(*depth as usize));
                }
                let empty = empty_hashes(domain);
                if *left == empty[*depth as usize + 1] && *right == empty[*depth as usize + 1] {
                    return Err(StateError::NonCanonicalEmptyBranch(*depth));
                }
                branch_hash(domain, *depth, *left, *right)
            }
        }
    }
}
