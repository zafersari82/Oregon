use std::collections::BTreeMap;

use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;

use crate::source::{StateSource, load_checked_node, load_checked_value};
use crate::{SMT_DEPTH, StateError, StateNode, empty_hashes, leaf_hash, path_bit, path_key, value_hash};

pub const MAX_STATE_WRITE_SET_ENTRIES: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainSnapshot {
    pub domain: CommitmentDomainId,
    pub root: Hash256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateWrite {
    key: Vec<u8>,
    value: Option<Vec<u8>>,
}

impl StateWrite {
    pub fn put(key: Vec<u8>, value: Vec<u8>) -> Self {
        Self {
            key,
            value: Some(value),
        }
    }

    pub fn delete(key: Vec<u8>) -> Self {
        Self { key, value: None }
    }

    pub fn key(&self) -> &[u8] {
        &self.key
    }

    pub fn value(&self) -> Option<&[u8]> {
        self.value.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreparedValue {
    Put { bytes: Vec<u8>, hash: Hash256 },
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedWrite {
    path: Hash256,
    value: PreparedValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateWriteSet {
    domain: CommitmentDomainId,
    writes: Vec<PreparedWrite>,
}

impl StateWriteSet {
    pub fn new(domain: CommitmentDomainId, writes: Vec<StateWrite>) -> Result<Self, StateError> {
        if writes.len() > MAX_STATE_WRITE_SET_ENTRIES {
            return Err(StateError::WriteSetTooLarge(writes.len()));
        }

        let mut prepared = Vec::new();
        for write in writes {
            let path = path_key(domain, &write.key)?;
            let value = match write.value {
                Some(bytes) => {
                    let hash = value_hash(domain, &bytes)?;
                    PreparedValue::Put { bytes, hash }
                }
                None => PreparedValue::Delete,
            };
            prepared.push(PreparedWrite { path, value });
        }

        prepared.sort_by_key(|write| write.path);
        for pair in prepared.windows(2) {
            if pair[0].path == pair[1].path {
                return Err(StateError::DuplicatePath(pair[0].path));
            }
        }

        Ok(Self {
            domain,
            writes: prepared,
        })
    }

    pub fn domain(&self) -> CommitmentDomainId {
        self.domain
    }

    pub fn len(&self) -> usize {
        self.writes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.writes.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateTransition {
    pub domain: CommitmentDomainId,
    pub old_root: Hash256,
    pub new_root: Hash256,
    pub nodes: BTreeMap<Hash256, StateNode>,
    pub values: BTreeMap<Hash256, Vec<u8>>,
}

pub fn apply_write_set<S: StateSource + ?Sized>(
    source: &S,
    snapshot: DomainSnapshot,
    write_set: &StateWriteSet,
) -> Result<StateTransition, StateError> {
    if snapshot.domain != write_set.domain {
        return Err(StateError::DomainMismatch);
    }

    if write_set.writes.is_empty() {
        return Ok(StateTransition {
            domain: snapshot.domain,
            old_root: snapshot.root,
            new_root: snapshot.root,
            nodes: BTreeMap::new(),
            values: BTreeMap::new(),
        });
    }

    let empty = empty_hashes(snapshot.domain);
    let mut builder = TransitionBuilder {
        source,
        domain: snapshot.domain,
        empty,
        nodes: BTreeMap::new(),
        values: BTreeMap::new(),
    };
    let new_root = builder.apply_subtree(snapshot.root, 0, &write_set.writes)?;

    Ok(StateTransition {
        domain: snapshot.domain,
        old_root: snapshot.root,
        new_root,
        nodes: builder.nodes,
        values: builder.values,
    })
}

pub fn read_value<S: StateSource + ?Sized>(
    source: &S,
    snapshot: DomainSnapshot,
    key: &[u8],
) -> Result<Option<Vec<u8>>, StateError> {
    let path = path_key(snapshot.domain, key)?;
    let empty = empty_hashes(snapshot.domain);
    let mut current = snapshot.root;

    for depth in 0..SMT_DEPTH {
        if current == empty[depth] {
            return Ok(None);
        }
        let node = load_checked_node(source, snapshot.domain, current, depth)?;
        let StateNode::Branch { left, right, .. } = node else {
            return Err(StateError::UnexpectedLeaf);
        };
        current = if path_bit(path, depth)? { right } else { left };
    }

    if current == empty[SMT_DEPTH] {
        return Ok(None);
    }

    let node = load_checked_node(source, snapshot.domain, current, SMT_DEPTH)?;
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

    Ok(Some(load_checked_value(
        source,
        snapshot.domain,
        committed_value_hash,
    )?))
}

struct TransitionBuilder<'a, S: StateSource + ?Sized> {
    source: &'a S,
    domain: CommitmentDomainId,
    empty: [Hash256; SMT_DEPTH + 1],
    nodes: BTreeMap<Hash256, StateNode>,
    values: BTreeMap<Hash256, Vec<u8>>,
}

impl<S: StateSource + ?Sized> TransitionBuilder<'_, S> {
    fn apply_subtree(
        &mut self,
        old_hash: Hash256,
        depth: usize,
        writes: &[PreparedWrite],
    ) -> Result<Hash256, StateError> {
        if writes.is_empty() {
            return Ok(old_hash);
        }

        if depth == SMT_DEPTH {
            return self.apply_leaf(old_hash, &writes[0]);
        }

        let (old_left, old_right) = if old_hash == self.empty[depth] {
            (self.empty[depth + 1], self.empty[depth + 1])
        } else {
            let node = load_checked_node(self.source, self.domain, old_hash, depth)?;
            let StateNode::Branch { left, right, .. } = node else {
                return Err(StateError::UnexpectedLeaf);
            };
            (left, right)
        };

        let split = writes.partition_point(|write| !path_bit_unchecked(write.path, depth));
        let new_left = if split == 0 {
            old_left
        } else {
            self.apply_subtree(old_left, depth + 1, &writes[..split])?
        };
        let new_right = if split == writes.len() {
            old_right
        } else {
            self.apply_subtree(old_right, depth + 1, &writes[split..])?
        };

        if new_left == old_left && new_right == old_right {
            return Ok(old_hash);
        }
        if new_left == self.empty[depth + 1] && new_right == self.empty[depth + 1] {
            return Ok(self.empty[depth]);
        }

        let node = StateNode::Branch {
            depth: depth as u16,
            left: new_left,
            right: new_right,
        };
        let hash = node.hash(self.domain)?;
        self.nodes.insert(hash, node);
        Ok(hash)
    }

    fn apply_leaf(
        &mut self,
        old_hash: Hash256,
        write: &PreparedWrite,
    ) -> Result<Hash256, StateError> {
        let old_value_hash = if old_hash == self.empty[SMT_DEPTH] {
            None
        } else {
            let node = load_checked_node(self.source, self.domain, old_hash, SMT_DEPTH)?;
            let StateNode::Leaf {
                path_key: actual_path,
                value_hash,
            } = node
            else {
                return Err(StateError::UnexpectedBranch);
            };
            if actual_path != write.path {
                return Err(StateError::LeafPathMismatch {
                    expected: write.path,
                    actual: actual_path,
                });
            }
            Some(value_hash)
        };

        match &write.value {
            PreparedValue::Delete => {
                if old_value_hash.is_none() {
                    Ok(old_hash)
                } else {
                    Ok(self.empty[SMT_DEPTH])
                }
            }
            PreparedValue::Put { bytes, hash } => {
                if old_value_hash == Some(*hash) {
                    return Ok(old_hash);
                }
                self.record_value(*hash, bytes)?;
                let node = StateNode::Leaf {
                    path_key: write.path,
                    value_hash: *hash,
                };
                let new_hash = leaf_hash(self.domain, write.path, *hash);
                self.nodes.insert(new_hash, node);
                Ok(new_hash)
            }
        }
    }

    fn record_value(&mut self, hash: Hash256, value: &[u8]) -> Result<(), StateError> {
        if let Some(existing) = self.source.get_value(&hash)? {
            if value_hash(self.domain, &existing)? != hash || existing.as_slice() != value {
                return Err(StateError::ValueHashMismatch(hash));
            }
            return Ok(());
        }
        self.values.entry(hash).or_insert_with(|| value.to_vec());
        Ok(())
    }
}

fn path_bit_unchecked(path: Hash256, depth: usize) -> bool {
    let byte = path.as_bytes()[depth / 8];
    (byte & (0x80 >> (depth % 8))) != 0
}
