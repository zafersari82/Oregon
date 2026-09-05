use std::str::FromStr;

use oregon_contract_state::{
    DomainSnapshot, MAX_STATE_KEY_BYTES, MAX_STATE_VALUE_BYTES, MAX_STATE_WRITE_SET_ENTRIES,
    SMT_DEPTH, StateError, StateNode, StateTransition, StateWrite, StateWriteSet, apply_write_set,
    empty_hashes, path_bit, path_key, read_value, value_hash,
};
use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;
use proptest::prelude::*;

mod support;
use support::MemorySource;

fn empty_snapshot(domain: CommitmentDomainId) -> DomainSnapshot {
    DomainSnapshot {
        domain,
        root: empty_hashes(domain)[0],
    }
}

fn writes(domain: CommitmentDomainId, writes: Vec<StateWrite>) -> StateWriteSet {
    StateWriteSet::new(domain, writes).unwrap()
}

fn single_alpha_transition(source: &MemorySource) -> StateTransition {
    let domain = CommitmentDomainId::Wasm;
    apply_write_set(
        source,
        empty_snapshot(domain),
        &writes(
            domain,
            vec![StateWrite::put(b"alpha".to_vec(), b"beta".to_vec())],
        ),
    )
    .unwrap()
}

fn populated_alpha_source() -> (MemorySource, DomainSnapshot, Hash256) {
    let domain = CommitmentDomainId::Wasm;
    let mut source = MemorySource::default();
    let transition = single_alpha_transition(&source);
    let committed_value_hash = *transition.values.keys().next().unwrap();
    let snapshot = DomainSnapshot {
        domain,
        root: transition.new_root,
    };
    source.absorb(&transition);
    (source, snapshot, committed_value_hash)
}

fn apply_alpha_write(
    source: &MemorySource,
    snapshot: DomainSnapshot,
    write: StateWrite,
) -> Result<StateTransition, StateError> {
    apply_write_set(source, snapshot, &writes(snapshot.domain, vec![write]))
}

#[test]
fn batch_write_order_is_canonical_and_matches_literal_root() {
    let domain = CommitmentDomainId::Wasm;
    let source = MemorySource::default();
    let expected =
        Hash256::from_str("0f44ec4ed4f99c010d6fb82e5f4afaf0513949bf3bddbc8022c56206b02fbb41")
            .unwrap();

    let first = writes(
        domain,
        vec![
            StateWrite::put(b"alpha".to_vec(), b"beta".to_vec()),
            StateWrite::put(b"omega".to_vec(), b"zeta".to_vec()),
        ],
    );
    let reversed = writes(
        domain,
        vec![
            StateWrite::put(b"omega".to_vec(), b"zeta".to_vec()),
            StateWrite::put(b"alpha".to_vec(), b"beta".to_vec()),
        ],
    );

    let left = apply_write_set(&source, empty_snapshot(domain), &first).unwrap();
    let right = apply_write_set(&source, empty_snapshot(domain), &reversed).unwrap();
    assert_eq!(left.new_root, expected);
    assert_eq!(right.new_root, expected);
    assert_eq!(left.nodes, right.nodes);
    assert_eq!(left.values, right.values);
}

#[test]
fn long_shared_prefix_batch_matches_literal_root() {
    let domain = CommitmentDomainId::Wasm;
    let source = MemorySource::default();
    let set = writes(
        domain,
        vec![
            StateWrite::put(b"alpha".to_vec(), b"beta".to_vec()),
            StateWrite::put(b"k38073".to_vec(), b"theta".to_vec()),
        ],
    );
    let transition = apply_write_set(&source, empty_snapshot(domain), &set).unwrap();
    assert_eq!(
        transition.new_root,
        Hash256::from_str("04a7764c5cc1df20cc93fd4ed8dc6dadb27926e7732d810baf821e2d1dfdb6b1")
            .unwrap()
    );
}

#[test]
fn update_delete_and_read_preserve_immutable_snapshot_semantics() {
    let domain = CommitmentDomainId::Wasm;
    let mut source = MemorySource::default();

    let initial = single_alpha_transition(&source);
    assert_eq!(
        initial.new_root,
        Hash256::from_str("3f61d102884e3577cccc9523f2178d76ebb597824c4f147669c1969a237b5553")
            .unwrap()
    );
    source.absorb(&initial);

    let initial_snapshot = DomainSnapshot {
        domain,
        root: initial.new_root,
    };
    assert_eq!(
        read_value(&source, initial_snapshot, b"alpha").unwrap(),
        Some(b"beta".to_vec())
    );

    let updated = apply_write_set(
        &source,
        initial_snapshot,
        &writes(
            domain,
            vec![StateWrite::put(b"alpha".to_vec(), b"gamma".to_vec())],
        ),
    )
    .unwrap();
    assert_eq!(
        updated.new_root,
        Hash256::from_str("ef26ec879a11d1a0e48a1649eeab4ef917783dbee4f6f87aa443b67d626123da")
            .unwrap()
    );
    assert_eq!(initial_snapshot.root, updated.old_root);
    let updated_snapshot = DomainSnapshot {
        domain,
        root: updated.new_root,
    };
    source.absorb(&updated);
    assert_eq!(
        read_value(&source, initial_snapshot, b"alpha").unwrap(),
        Some(b"beta".to_vec())
    );
    assert_eq!(
        read_value(&source, updated_snapshot, b"alpha").unwrap(),
        Some(b"gamma".to_vec())
    );

    let deleted = apply_write_set(
        &source,
        initial_snapshot,
        &writes(domain, vec![StateWrite::delete(b"alpha".to_vec())]),
    )
    .unwrap();
    assert_eq!(deleted.new_root, empty_hashes(domain)[0]);
}

#[test]
fn delete_absent_and_same_value_put_are_deterministic_noops() {
    let domain = CommitmentDomainId::Wasm;
    let mut source = MemorySource::default();
    let empty = empty_snapshot(domain);

    let deleted = apply_write_set(
        &source,
        empty,
        &writes(domain, vec![StateWrite::delete(b"missing".to_vec())]),
    )
    .unwrap();
    assert_eq!(deleted.old_root, deleted.new_root);
    assert!(deleted.nodes.is_empty());
    assert!(deleted.values.is_empty());

    let initial = single_alpha_transition(&source);
    source.absorb(&initial);
    let snapshot = DomainSnapshot {
        domain,
        root: initial.new_root,
    };
    let same = apply_write_set(
        &source,
        snapshot,
        &writes(
            domain,
            vec![StateWrite::put(b"alpha".to_vec(), b"beta".to_vec())],
        ),
    )
    .unwrap();
    assert_eq!(same.old_root, same.new_root);
    assert!(same.nodes.is_empty());
    assert!(same.values.is_empty());
}

#[test]
fn present_empty_value_round_trips_and_deletes_to_absence() {
    let domain = CommitmentDomainId::Wasm;
    let mut source = MemorySource::default();
    let inserted = apply_write_set(
        &source,
        empty_snapshot(domain),
        &writes(domain, vec![StateWrite::put(b"empty".to_vec(), Vec::new())]),
    )
    .unwrap();
    assert_ne!(inserted.new_root, empty_hashes(domain)[0]);
    source.absorb(&inserted);

    let populated = DomainSnapshot {
        domain,
        root: inserted.new_root,
    };
    assert_eq!(
        read_value(&source, populated, b"empty").unwrap(),
        Some(Vec::new())
    );

    let deleted = apply_write_set(
        &source,
        populated,
        &writes(domain, vec![StateWrite::delete(b"empty".to_vec())]),
    )
    .unwrap();
    assert_eq!(deleted.new_root, empty_hashes(domain)[0]);
}

#[test]
fn deletion_rejects_missing_old_value_blob() {
    let (mut source, snapshot, old_value_hash) = populated_alpha_source();
    source.values.remove(&old_value_hash);

    assert!(matches!(
        apply_alpha_write(&source, snapshot, StateWrite::delete(b"alpha".to_vec())),
        Err(StateError::MissingValue(hash)) if hash == old_value_hash
    ));
}

#[test]
fn deletion_rejects_corrupt_old_value_blob() {
    let (mut source, snapshot, old_value_hash) = populated_alpha_source();
    source.values.insert(old_value_hash, b"tampered".to_vec());

    assert!(matches!(
        apply_alpha_write(&source, snapshot, StateWrite::delete(b"alpha".to_vec())),
        Err(StateError::ValueHashMismatch(hash)) if hash == old_value_hash
    ));
}

#[test]
fn replacement_rejects_missing_old_value_blob() {
    let (mut source, snapshot, old_value_hash) = populated_alpha_source();
    source.values.remove(&old_value_hash);

    assert!(matches!(
        apply_alpha_write(
            &source,
            snapshot,
            StateWrite::put(b"alpha".to_vec(), b"gamma".to_vec()),
        ),
        Err(StateError::MissingValue(hash)) if hash == old_value_hash
    ));
}

#[test]
fn replacement_rejects_corrupt_old_value_blob() {
    let (mut source, snapshot, old_value_hash) = populated_alpha_source();
    source.values.insert(old_value_hash, b"tampered".to_vec());

    assert!(matches!(
        apply_alpha_write(
            &source,
            snapshot,
            StateWrite::put(b"alpha".to_vec(), b"gamma".to_vec()),
        ),
        Err(StateError::ValueHashMismatch(hash)) if hash == old_value_hash
    ));
}

#[test]
fn same_value_put_rejects_missing_old_value_blob() {
    let (mut source, snapshot, old_value_hash) = populated_alpha_source();
    source.values.remove(&old_value_hash);

    assert!(matches!(
        apply_alpha_write(
            &source,
            snapshot,
            StateWrite::put(b"alpha".to_vec(), b"beta".to_vec()),
        ),
        Err(StateError::MissingValue(hash)) if hash == old_value_hash
    ));
}

#[test]
fn same_value_put_rejects_corrupt_old_value_blob() {
    let (mut source, snapshot, old_value_hash) = populated_alpha_source();
    source.values.insert(old_value_hash, b"tampered".to_vec());

    assert!(matches!(
        apply_alpha_write(
            &source,
            snapshot,
            StateWrite::put(b"alpha".to_vec(), b"beta".to_vec()),
        ),
        Err(StateError::ValueHashMismatch(hash)) if hash == old_value_hash
    ));
}

#[test]
fn duplicate_path_and_domain_mismatch_fail_closed() {
    let domain = CommitmentDomainId::Wasm;
    assert!(matches!(
        StateWriteSet::new(
            domain,
            vec![
                StateWrite::put(b"alpha".to_vec(), b"one".to_vec()),
                StateWrite::put(b"alpha".to_vec(), b"two".to_vec()),
            ],
        ),
        Err(StateError::DuplicatePath(_))
    ));

    let source = MemorySource::default();
    let accounting = writes(
        CommitmentDomainId::ExecutionAccounting,
        vec![StateWrite::put(b"alpha".to_vec(), b"beta".to_vec())],
    );
    assert!(matches!(
        apply_write_set(&source, empty_snapshot(domain), &accounting),
        Err(StateError::DomainMismatch)
    ));
}

#[test]
fn missing_nonempty_node_is_corruption_not_empty_state() {
    let domain = CommitmentDomainId::Wasm;
    let source = MemorySource::default();
    let bogus_root = Hash256::from_bytes([0x77; 32]);
    let snapshot = DomainSnapshot {
        domain,
        root: bogus_root,
    };
    let set = writes(
        domain,
        vec![StateWrite::put(b"alpha".to_vec(), b"beta".to_vec())],
    );

    assert!(matches!(
        apply_write_set(&source, snapshot, &set),
        Err(StateError::MissingNode(hash)) if hash == bogus_root
    ));
    assert!(matches!(
        read_value(&source, snapshot, b"alpha"),
        Err(StateError::MissingNode(hash)) if hash == bogus_root
    ));
}

#[test]
fn wrong_node_hash_fails_closed() {
    let domain = CommitmentDomainId::Wasm;
    let empty = empty_hashes(domain);
    let mut source = MemorySource::default();
    let requested = Hash256::from_bytes([0x11; 32]);
    source.nodes.insert(
        requested,
        StateNode::Branch {
            depth: 0,
            left: empty[1],
            right: Hash256::from_bytes([0x22; 32]),
        },
    );

    let snapshot = DomainSnapshot {
        domain,
        root: requested,
    };
    assert!(matches!(
        read_value(&source, snapshot, b"alpha"),
        Err(StateError::NodeHashMismatch(hash)) if hash == requested
    ));
}

#[test]
fn wrong_branch_depth_fails_closed() {
    let domain = CommitmentDomainId::Wasm;
    let empty = empty_hashes(domain);
    let mut source = MemorySource::default();
    let node = StateNode::Branch {
        depth: 1,
        left: empty[2],
        right: Hash256::from_bytes([0x33; 32]),
    };
    let root = node.hash(domain).unwrap();
    source.nodes.insert(root, node);

    assert!(matches!(
        read_value(&source, DomainSnapshot { domain, root }, b"alpha"),
        Err(StateError::NodeDepthMismatch {
            expected: 0,
            actual: 1
        })
    ));
}

#[test]
fn leaf_at_internal_depth_fails_closed() {
    let domain = CommitmentDomainId::Wasm;
    let path = path_key(domain, b"alpha").unwrap();
    let committed_value = value_hash(domain, b"beta").unwrap();
    let leaf = StateNode::Leaf {
        path_key: path,
        value_hash: committed_value,
    };
    let root = leaf.hash(domain).unwrap();
    let mut source = MemorySource::default();
    source.nodes.insert(root, leaf);

    assert!(matches!(
        read_value(&source, DomainSnapshot { domain, root }, b"alpha"),
        Err(StateError::UnexpectedLeaf)
    ));
}

#[test]
fn branch_at_leaf_depth_fails_closed() {
    let domain = CommitmentDomainId::Wasm;
    let empty = empty_hashes(domain);
    let path = path_key(domain, b"alpha").unwrap();
    let mut source = MemorySource::default();

    let terminal_branch = StateNode::Branch {
        depth: 0,
        left: empty[1],
        right: Hash256::from_bytes([0x44; 32]),
    };
    let mut child = terminal_branch.hash(domain).unwrap();
    source.nodes.insert(child, terminal_branch);

    for depth in (0..SMT_DEPTH).rev() {
        let sibling = empty[depth + 1];
        let node = if path_bit(path, depth).unwrap() {
            StateNode::Branch {
                depth: depth as u16,
                left: sibling,
                right: child,
            }
        } else {
            StateNode::Branch {
                depth: depth as u16,
                left: child,
                right: sibling,
            }
        };
        child = node.hash(domain).unwrap();
        source.nodes.insert(child, node);
    }

    assert!(matches!(
        read_value(
            &source,
            DomainSnapshot {
                domain,
                root: child,
            },
            b"alpha",
        ),
        Err(StateError::UnexpectedBranch)
    ));
}

#[test]
fn missing_and_corrupt_value_blobs_fail_closed() {
    let domain = CommitmentDomainId::Wasm;
    let empty_source = MemorySource::default();
    let transition = single_alpha_transition(&empty_source);
    let committed_value_hash = *transition.values.keys().next().unwrap();

    let mut source = MemorySource::default();
    source.nodes.extend(
        transition
            .nodes
            .iter()
            .map(|(hash, node)| (*hash, node.clone())),
    );
    let snapshot = DomainSnapshot {
        domain,
        root: transition.new_root,
    };
    assert!(matches!(
        read_value(&source, snapshot, b"alpha"),
        Err(StateError::MissingValue(hash)) if hash == committed_value_hash
    ));

    source
        .values
        .insert(committed_value_hash, b"tampered".to_vec());
    assert!(matches!(
        read_value(&source, snapshot, b"alpha"),
        Err(StateError::ValueHashMismatch(hash)) if hash == committed_value_hash
    ));
}

#[test]
fn state_key_and_value_boundaries_accept_limit_and_reject_limit_plus_one() {
    let domain = CommitmentDomainId::Wasm;
    assert!(path_key(domain, &vec![0u8; MAX_STATE_KEY_BYTES]).is_ok());
    assert!(matches!(
        path_key(domain, &vec![0u8; MAX_STATE_KEY_BYTES + 1]),
        Err(StateError::KeyTooLarge(size)) if size == MAX_STATE_KEY_BYTES + 1
    ));

    assert!(value_hash(domain, &vec![0u8; MAX_STATE_VALUE_BYTES]).is_ok());
    assert!(matches!(
        value_hash(domain, &vec![0u8; MAX_STATE_VALUE_BYTES + 1]),
        Err(StateError::ValueTooLarge(size)) if size == MAX_STATE_VALUE_BYTES + 1
    ));
}

#[test]
fn write_set_boundary_accepts_limit_and_rejects_limit_plus_one() {
    let domain = CommitmentDomainId::Wasm;
    let maximum = (0..MAX_STATE_WRITE_SET_ENTRIES)
        .map(|index| StateWrite::delete((index as u64).to_be_bytes().to_vec()))
        .collect();
    let set = StateWriteSet::new(domain, maximum).unwrap();
    assert_eq!(set.len(), MAX_STATE_WRITE_SET_ENTRIES);

    let oversized = vec![StateWrite::delete(Vec::new()); MAX_STATE_WRITE_SET_ENTRIES + 1];
    assert!(matches!(
        StateWriteSet::new(domain, oversized),
        Err(StateError::WriteSetTooLarge(size)) if size == MAX_STATE_WRITE_SET_ENTRIES + 1
    ));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn random_small_key_value_batches_are_order_independent(
        entries in prop::collection::vec(
            (
                prop::collection::vec(any::<u8>(), 0..=32),
                prop::option::of(prop::collection::vec(any::<u8>(), 0..=64)),
            ),
            0..=24,
        ),
    ) {
        let to_write = |(key, value): &(Vec<u8>, Option<Vec<u8>>)| match value {
            Some(value) => StateWrite::put(key.clone(), value.clone()),
            None => StateWrite::delete(key.clone()),
        };
        let forward = StateWriteSet::new(
            CommitmentDomainId::Wasm,
            entries.iter().map(to_write).collect(),
        );
        let reversed = StateWriteSet::new(
            CommitmentDomainId::Wasm,
            entries.iter().rev().map(to_write).collect(),
        );

        match (forward, reversed) {
            (Ok(forward), Ok(reversed)) => {
                let source = MemorySource::default();
                let snapshot = empty_snapshot(CommitmentDomainId::Wasm);
                let forward = apply_write_set(&source, snapshot, &forward).unwrap();
                let reversed = apply_write_set(&source, snapshot, &reversed).unwrap();
                prop_assert_eq!(forward.new_root, reversed.new_root);
                prop_assert_eq!(forward.nodes, reversed.nodes);
                prop_assert_eq!(forward.values, reversed.values);
            }
            (Err(StateError::DuplicatePath(forward)), Err(StateError::DuplicatePath(reversed))) => {
                prop_assert_eq!(forward, reversed);
            }
            (forward, reversed) => {
                prop_assert!(false, "normalization disagreed: {forward:?} versus {reversed:?}");
            }
        }
    }
}
