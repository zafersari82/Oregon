use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;

use crate::{
    DomainSnapshot, SMT_DEPTH, StateError, StateNode, StateSource, branch_hash, empty_hashes,
    leaf_hash, path_bit, path_key, value_hash,
};

pub const SMT_PROOF_VERSION: u16 = 1;
pub const SMT_PROOF_BITMAP_BYTES: usize = 32;
pub const MAX_SMT_SIBLINGS: usize = 256;
pub const MAX_SMT_PROOF_BYTES: usize =
    2 + SMT_PROOF_BITMAP_BYTES + MAX_SMT_SIBLINGS * 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseMerkleProofV1 {
    sibling_bitmap: [u8; SMT_PROOF_BITMAP_BYTES],
    siblings: Vec<Hash256>,
}

impl SparseMerkleProofV1 {
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(2 + SMT_PROOF_BITMAP_BYTES + self.siblings.len() * 32);
        bytes.extend_from_slice(&SMT_PROOF_VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.sibling_bitmap);
        for sibling in &self.siblings {
            bytes.extend_from_slice(sibling.as_bytes());
        }
        bytes
    }

    pub fn decode(domain: CommitmentDomainId, bytes: &[u8]) -> Result<Self, StateError> {
        const HEADER_BYTES: usize = 2 + SMT_PROOF_BITMAP_BYTES;
        if bytes.len() > MAX_SMT_PROOF_BYTES {
            return Err(StateError::ProofTooLarge(bytes.len()));
        }
        if bytes.len() < HEADER_BYTES || (bytes.len() - HEADER_BYTES) % 32 != 0 {
            return Err(StateError::MalformedProof);
        }

        let version = u16::from_le_bytes([bytes[0], bytes[1]]);
        if version != SMT_PROOF_VERSION {
            return Err(StateError::MalformedProof);
        }

        let mut sibling_bitmap = [0u8; SMT_PROOF_BITMAP_BYTES];
        sibling_bitmap.copy_from_slice(&bytes[2..HEADER_BYTES]);
        let expected_siblings = sibling_bitmap
            .iter()
            .map(|byte| byte.count_ones() as usize)
            .sum::<usize>();
        let encoded_siblings = (bytes.len() - HEADER_BYTES) / 32;
        if encoded_siblings != expected_siblings || encoded_siblings > MAX_SMT_SIBLINGS {
            return Err(StateError::MalformedProof);
        }

        let empty = empty_hashes(domain);
        let mut siblings = Vec::new();
        let mut offset = HEADER_BYTES;
        for depth in 0..SMT_DEPTH {
            if !bitmap_bit(&sibling_bitmap, depth) {
                continue;
            }
            let mut hash_bytes = [0u8; 32];
            hash_bytes.copy_from_slice(&bytes[offset..offset + 32]);
            offset += 32;
            let sibling = Hash256::from_bytes(hash_bytes);
            if sibling == empty[depth + 1] {
                return Err(StateError::RedundantDefaultSibling(depth));
            }
            siblings.push(sibling);
        }

        if offset != bytes.len() {
            return Err(StateError::MalformedProof);
        }

        Ok(Self {
            sibling_bitmap,
            siblings,
        })
    }
}

pub fn prove<S: StateSource + ?Sized>(
    source: &S,
    snapshot: DomainSnapshot,
    key: &[u8],
) -> Result<(Option<Vec<u8>>, SparseMerkleProofV1), StateError> {
    let path = path_key(snapshot.domain, key)?;
    let empty = empty_hashes(snapshot.domain);
    let mut current = snapshot.root;
    let mut sibling_bitmap = [0u8; SMT_PROOF_BITMAP_BYTES];
    let mut siblings = Vec::new();

    for depth in 0..SMT_DEPTH {
        if current == empty[depth] {
            return Ok((
                None,
                SparseMerkleProofV1 {
                    sibling_bitmap,
                    siblings,
                },
            ));
        }

        let node = load_checked_node(source, snapshot.domain, &empty, current, depth)?;
        let StateNode::Branch { left, right, .. } = node else {
            return Err(StateError::UnexpectedLeaf);
        };

        let goes_right = path_bit(path, depth)?;
        let (next, sibling) = if goes_right {
            (right, left)
        } else {
            (left, right)
        };
        if sibling != empty[depth + 1] {
            set_bitmap_bit(&mut sibling_bitmap, depth);
            siblings.push(sibling);
        }
        current = next;
    }

    if current == empty[SMT_DEPTH] {
        return Ok((
            None,
            SparseMerkleProofV1 {
                sibling_bitmap,
                siblings,
            },
        ));
    }

    let node = load_checked_node(
        source,
        snapshot.domain,
        &empty,
        current,
        SMT_DEPTH,
    )?;
    let StateNode::Leaf {
        path_key: actual_path,
        value_hash: committed_value_hash,
    } = node
    else {
        return Err(StateError::UnexpectedBranch);
    };
    if actual_path != path {
        return Err(StateError::LeafPathMismatch {
            expected: path,
            actual: actual_path,
        });
    }

    let value = source
        .get_value(&committed_value_hash)?
        .ok_or(StateError::MissingValue(committed_value_hash))?;
    if value_hash(snapshot.domain, &value)? != committed_value_hash {
        return Err(StateError::ValueHashMismatch(committed_value_hash));
    }

    Ok((
        Some(value),
        SparseMerkleProofV1 {
            sibling_bitmap,
            siblings,
        },
    ))
}

pub fn verify_proof(
    domain: CommitmentDomainId,
    key: &[u8],
    value: Option<&[u8]>,
    proof: &SparseMerkleProofV1,
    expected_root: Hash256,
) -> Result<(), StateError> {
    let path = path_key(domain, key)?;
    let empty = empty_hashes(domain);
    let mut current = match value {
        Some(value) => {
            let committed_value = value_hash(domain, value)?;
            leaf_hash(domain, path, committed_value)
        }
        None => empty[SMT_DEPTH],
    };

    let mut sibling_index = proof.siblings.len();
    for depth in (0..SMT_DEPTH).rev() {
        let sibling = if bitmap_bit(&proof.sibling_bitmap, depth) {
            if sibling_index == 0 {
                return Err(StateError::MalformedProof);
            }
            sibling_index -= 1;
            proof.siblings[sibling_index]
        } else {
            empty[depth + 1]
        };

        current = if path_bit(path, depth)? {
            branch_hash(domain, depth as u16, sibling, current)?
        } else {
            branch_hash(domain, depth as u16, current, sibling)?
        };
    }

    if sibling_index != 0 {
        return Err(StateError::MalformedProof);
    }
    if current != expected_root {
        return Err(StateError::InvalidProof);
    }
    Ok(())
}

fn load_checked_node<S: StateSource + ?Sized>(
    source: &S,
    domain: CommitmentDomainId,
    empty: &[Hash256; SMT_DEPTH + 1],
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
        StateNode::Branch {
            depth: actual,
            left,
            right,
        } if depth < SMT_DEPTH => {
            if *actual as usize != depth {
                return Err(StateError::NodeDepthMismatch {
                    expected: depth as u16,
                    actual: *actual,
                });
            }
            if *left == empty[depth + 1] && *right == empty[depth + 1] {
                return Err(StateError::NonCanonicalEmptyBranch(*actual));
            }
            Ok(node)
        }
        StateNode::Leaf { .. } if depth < SMT_DEPTH => Err(StateError::UnexpectedLeaf),
        StateNode::Branch { .. } => Err(StateError::UnexpectedBranch),
        StateNode::Leaf { .. } => Ok(node),
    }
}

fn bitmap_bit(bitmap: &[u8; SMT_PROOF_BITMAP_BYTES], depth: usize) -> bool {
    (bitmap[depth / 8] & (0x80 >> (depth % 8))) != 0
}

fn set_bitmap_bit(bitmap: &mut [u8; SMT_PROOF_BITMAP_BYTES], depth: usize) {
    bitmap[depth / 8] |= 0x80 >> (depth % 8);
}
