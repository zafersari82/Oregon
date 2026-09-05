use std::collections::BTreeMap;
use std::str::FromStr;

use oregon_contract_state::{
    DomainSnapshot, StateError, StateNode, StateSource, StateTransition, StateWrite, StateWriteSet,
    apply_write_set, empty_hashes, read_value,
};
use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;

#[derive(Default)]
struct MemorySource {
    nodes: BTreeMap<Hash256, StateNode>,
    values: BTreeMap<Hash256, Vec<u8>>,
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
    fn absorb(&mut self, transition: &StateTransition) {
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

fn empty_snapshot(domain: CommitmentDomainId) -> DomainSnapshot {
    DomainSnapshot {
        domain,
        root: empty_hashes(domain)[0],
    }
}

fn writes(domain: CommitmentDomainId, writes: Vec<StateWrite>) -> StateWriteSet {
    StateWriteSet::new(domain, writes).unwrap()
}

#[test]
fn batch_write_order_is_canonical_and_matches_literal_root() {
    let domain = CommitmentDomainId::Wasm;
    let source = MemorySource::default();
    let expected = Hash256::from_str(
        "0f44ec4ed4f99c010d6fb82e5f4afaf0513949bf3bddbc8022c56206b02fbb41",
    )
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

    let initial = apply_write_set(
        &source,
        empty_snapshot(domain),
        &writes(
            domain,
            vec![StateWrite::put(b"alpha".to_vec(), b"beta".to_vec())],
        ),
    )
    .unwrap();
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

    let initial = apply_write_set(
        &source,
        empty,
        &writes(
            domain,
            vec![StateWrite::put(b"alpha".to_vec(), b"beta".to_vec())],
        ),
    )
    .unwrap();
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
