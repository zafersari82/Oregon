use std::collections::BTreeMap;

use oregon_contract_state::{
    DomainSnapshot, SparseMerkleProofV1, StateError, StateNode, StateSource, StateTransition,
    StateWrite, StateWriteSet, apply_write_set, empty_hashes, prove, verify_proof,
};
use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;

const SINGLE_ALPHA_MEMBERSHIP_HEX: &str =
    "01000000000000000000000000000000000000000000000000000000000000000000";
const EMPTY_MISSING_NONMEMBERSHIP_HEX: &str =
    "01000000000000000000000000000000000000000000000000000000000000000000";
const TRIPLE_ALPHA_MEMBERSHIP_HEX: &str = "0100200080000000000000000000000000000000000000000000000000000000000054640ff819faf4d395c56a3980c30de67a52dadd432bc1043c6b3ec24b23fa7f3c7fca5980be2aff39ab5ff09f334615a5eebf194d8a07a64aabd2b56786e2f5";
const TRIPLE_MISSING_NONMEMBERSHIP_HEX: &str = "01003000000000000000000000000000000000000000000000000000000000000000d3f8e15329761de2a7ce80a1c77bad39af30ec5b629d1886719a2d4b3633163204ad4bf658c337319fbe4f3b69076ce52825b8ab98fb5c212c336c8c760ba34d";

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

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0]);
            let low = hex_nibble(pair[1]);
            (high << 4) | low
        })
        .collect()
}

fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("non-canonical hex test vector"),
    }
}

fn empty_snapshot(domain: CommitmentDomainId) -> DomainSnapshot {
    DomainSnapshot {
        domain,
        root: empty_hashes(domain)[0],
    }
}

fn write_set(domain: CommitmentDomainId, writes: Vec<StateWrite>) -> StateWriteSet {
    StateWriteSet::new(domain, writes).unwrap()
}

fn single_alpha_state() -> (MemorySource, DomainSnapshot) {
    let domain = CommitmentDomainId::Wasm;
    let mut source = MemorySource::default();
    let transition = apply_write_set(
        &source,
        empty_snapshot(domain),
        &write_set(
            domain,
            vec![StateWrite::put(b"alpha".to_vec(), b"beta".to_vec())],
        ),
    )
    .unwrap();
    let snapshot = DomainSnapshot {
        domain,
        root: transition.new_root,
    };
    source.absorb(&transition);
    (source, snapshot)
}

fn triple_state() -> (MemorySource, DomainSnapshot) {
    let domain = CommitmentDomainId::Wasm;
    let mut source = MemorySource::default();
    let transition = apply_write_set(
        &source,
        empty_snapshot(domain),
        &write_set(
            domain,
            vec![
                StateWrite::put(b"alpha".to_vec(), b"beta".to_vec()),
                StateWrite::put(b"omega".to_vec(), b"zeta".to_vec()),
                StateWrite::put(b"k38073".to_vec(), b"theta".to_vec()),
            ],
        ),
    )
    .unwrap();
    let snapshot = DomainSnapshot {
        domain,
        root: transition.new_root,
    };
    source.absorb(&transition);
    (source, snapshot)
}

#[test]
fn literal_single_membership_and_empty_nonmembership_proofs_are_canonical() {
    let domain = CommitmentDomainId::Wasm;
    let (source, snapshot) = single_alpha_state();
    let (value, proof) = prove(&source, snapshot, b"alpha").unwrap();
    assert_eq!(value.as_deref(), Some(b"beta".as_slice()));
    assert_eq!(proof.encode(), decode_hex(SINGLE_ALPHA_MEMBERSHIP_HEX));

    let decoded = SparseMerkleProofV1::decode(domain, &decode_hex(SINGLE_ALPHA_MEMBERSHIP_HEX))
        .unwrap();
    verify_proof(domain, b"alpha", Some(b"beta"), &decoded, snapshot.root).unwrap();

    let empty_source = MemorySource::default();
    let empty = empty_snapshot(domain);
    let (missing, proof) = prove(&empty_source, empty, b"missing").unwrap();
    assert_eq!(missing, None);
    assert_eq!(proof.encode(), decode_hex(EMPTY_MISSING_NONMEMBERSHIP_HEX));

    let decoded = SparseMerkleProofV1::decode(
        domain,
        &decode_hex(EMPTY_MISSING_NONMEMBERSHIP_HEX),
    )
    .unwrap();
    verify_proof(domain, b"missing", None, &decoded, empty.root).unwrap();
}

#[test]
fn literal_multi_sibling_membership_proof_matches_independent_vector() {
    let domain = CommitmentDomainId::Wasm;
    let (source, snapshot) = triple_state();
    let (value, proof) = prove(&source, snapshot, b"alpha").unwrap();
    assert_eq!(value.as_deref(), Some(b"beta".as_slice()));
    assert_eq!(proof.encode(), decode_hex(TRIPLE_ALPHA_MEMBERSHIP_HEX));

    let decoded = SparseMerkleProofV1::decode(domain, &decode_hex(TRIPLE_ALPHA_MEMBERSHIP_HEX))
        .unwrap();
    assert_eq!(decoded.encode(), decode_hex(TRIPLE_ALPHA_MEMBERSHIP_HEX));
    verify_proof(domain, b"alpha", Some(b"beta"), &decoded, snapshot.root).unwrap();

    let missing = SparseMerkleProofV1::decode(
        domain,
        &decode_hex(TRIPLE_MISSING_NONMEMBERSHIP_HEX),
    )
    .unwrap();
    assert_eq!(
        missing.encode(),
        decode_hex(TRIPLE_MISSING_NONMEMBERSHIP_HEX)
    );
}

#[test]
fn proof_decoder_rejects_malformed_and_redundant_default_siblings() {
    let domain = CommitmentDomainId::Wasm;
    let canonical = decode_hex(SINGLE_ALPHA_MEMBERSHIP_HEX);

    assert!(SparseMerkleProofV1::decode(domain, &canonical[..33]).is_err());

    let mut unsupported_version = canonical.clone();
    unsupported_version[0] = 2;
    assert!(SparseMerkleProofV1::decode(domain, &unsupported_version).is_err());

    let mut missing_explicit_sibling = canonical.clone();
    missing_explicit_sibling[2] = 0x80;
    assert!(SparseMerkleProofV1::decode(domain, &missing_explicit_sibling).is_err());

    let mut redundant_default = Vec::with_capacity(66);
    redundant_default.extend_from_slice(&1u16.to_le_bytes());
    let mut bitmap = [0u8; 32];
    bitmap[0] = 0x80;
    redundant_default.extend_from_slice(&bitmap);
    redundant_default.extend_from_slice(empty_hashes(domain)[1].as_bytes());
    assert!(matches!(
        SparseMerkleProofV1::decode(domain, &redundant_default),
        Err(StateError::RedundantDefaultSibling(0))
    ));
}

#[test]
fn proof_verification_binds_domain_key_value_and_root() {
    let domain = CommitmentDomainId::Wasm;
    let (_, snapshot) = single_alpha_state();
    let proof = SparseMerkleProofV1::decode(domain, &decode_hex(SINGLE_ALPHA_MEMBERSHIP_HEX))
        .unwrap();

    verify_proof(domain, b"alpha", Some(b"beta"), &proof, snapshot.root).unwrap();
    assert!(verify_proof(
        CommitmentDomainId::ExecutionAccounting,
        b"alpha",
        Some(b"beta"),
        &proof,
        snapshot.root,
    )
    .is_err());
    assert!(verify_proof(domain, b"omega", Some(b"beta"), &proof, snapshot.root).is_err());
    assert!(verify_proof(domain, b"alpha", Some(b"gamma"), &proof, snapshot.root).is_err());
    assert!(verify_proof(
        domain,
        b"alpha",
        Some(b"beta"),
        &proof,
        empty_hashes(domain)[0],
    )
    .is_err());
}

#[test]
fn proof_construction_fails_closed_on_missing_nonempty_node() {
    let domain = CommitmentDomainId::Wasm;
    let source = MemorySource::default();
    let bogus_root = Hash256::from_bytes([0x99; 32]);
    let snapshot = DomainSnapshot {
        domain,
        root: bogus_root,
    };

    assert!(matches!(
        prove(&source, snapshot, b"alpha"),
        Err(StateError::MissingNode(hash)) if hash == bogus_root
    ));
}
