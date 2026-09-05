use oregon_contract_state::{
    DomainSnapshot, MAX_SMT_PROOF_BYTES, SparseMerkleProofV1, StateError, StateNode, StateWrite,
    StateWriteSet, apply_write_set, empty_hashes, prove, verify_proof,
};
use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;
use proptest::prelude::*;

const SINGLE_ALPHA_MEMBERSHIP_HEX: &str =
    "01000000000000000000000000000000000000000000000000000000000000000000";
const EMPTY_MISSING_NONMEMBERSHIP_HEX: &str =
    "01000000000000000000000000000000000000000000000000000000000000000000";
const TRIPLE_ALPHA_MEMBERSHIP_HEX: &str = "0100200080000000000000000000000000000000000000000000000000000000000054640ff819faf4d395c56a3980c30de67a52dadd432bc1043c6b3ec24b23fa7f3c7fca5980be2aff39ab5ff09f334615a5eebf194d8a07a64aabd2b56786e2f5";
const TRIPLE_MISSING_NONMEMBERSHIP_HEX: &str = "01003000000000000000000000000000000000000000000000000000000000000000d3f8e15329761de2a7ce80a1c77bad39af30ec5b629d1886719a2d4b3633163204ad4bf658c337319fbe4f3b69076ce52825b8ab98fb5c212c336c8c760ba34d";

mod support;
use support::MemorySource;

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

    let decoded =
        SparseMerkleProofV1::decode(domain, &decode_hex(SINGLE_ALPHA_MEMBERSHIP_HEX)).unwrap();
    verify_proof(domain, b"alpha", Some(b"beta"), &decoded, snapshot.root).unwrap();

    let empty_source = MemorySource::default();
    let empty = empty_snapshot(domain);
    let (missing, proof) = prove(&empty_source, empty, b"missing").unwrap();
    assert_eq!(missing, None);
    assert_eq!(proof.encode(), decode_hex(EMPTY_MISSING_NONMEMBERSHIP_HEX));

    let decoded =
        SparseMerkleProofV1::decode(domain, &decode_hex(EMPTY_MISSING_NONMEMBERSHIP_HEX)).unwrap();
    verify_proof(domain, b"missing", None, &decoded, empty.root).unwrap();
}

#[test]
fn literal_multi_sibling_membership_proof_matches_independent_vector() {
    let domain = CommitmentDomainId::Wasm;
    let (source, snapshot) = triple_state();
    let (value, proof) = prove(&source, snapshot, b"alpha").unwrap();
    assert_eq!(value.as_deref(), Some(b"beta".as_slice()));
    assert_eq!(proof.encode(), decode_hex(TRIPLE_ALPHA_MEMBERSHIP_HEX));

    let decoded =
        SparseMerkleProofV1::decode(domain, &decode_hex(TRIPLE_ALPHA_MEMBERSHIP_HEX)).unwrap();
    assert_eq!(decoded.encode(), decode_hex(TRIPLE_ALPHA_MEMBERSHIP_HEX));
    verify_proof(domain, b"alpha", Some(b"beta"), &decoded, snapshot.root).unwrap();
}

#[test]
fn literal_multi_sibling_nonmembership_proof_is_constructed_and_verified() {
    let domain = CommitmentDomainId::Wasm;
    let (source, snapshot) = triple_state();
    let literal = decode_hex(TRIPLE_MISSING_NONMEMBERSHIP_HEX);

    let (value, constructed) = prove(&source, snapshot, b"missing").unwrap();
    assert_eq!(value, None);
    assert_eq!(constructed.encode(), literal);

    let decoded = SparseMerkleProofV1::decode(domain, &literal).unwrap();
    verify_proof(domain, b"missing", None, &decoded, snapshot.root).unwrap();
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

    let mut trailing = canonical.clone();
    trailing.push(0);
    assert!(matches!(
        SparseMerkleProofV1::decode(domain, &trailing),
        Err(StateError::MalformedProof)
    ));
}

#[test]
fn proof_size_boundary_accepts_exact_limit_and_rejects_limit_plus_one() {
    let domain = CommitmentDomainId::Wasm;
    let mut maximum = Vec::with_capacity(MAX_SMT_PROOF_BYTES);
    maximum.extend_from_slice(&1u16.to_le_bytes());
    maximum.extend_from_slice(&[0xff; 32]);
    for depth in 0..256 {
        let mut sibling = [0xa5; 32];
        sibling[0] = depth as u8;
        sibling[1] = (depth >> 8) as u8;
        maximum.extend_from_slice(&sibling);
    }
    assert_eq!(maximum.len(), MAX_SMT_PROOF_BYTES);
    assert_eq!(
        SparseMerkleProofV1::decode(domain, &maximum)
            .unwrap()
            .encode(),
        maximum
    );

    let oversized = vec![0u8; MAX_SMT_PROOF_BYTES + 1];
    assert!(matches!(
        SparseMerkleProofV1::decode(domain, &oversized),
        Err(StateError::ProofTooLarge(size)) if size == MAX_SMT_PROOF_BYTES + 1
    ));
}

#[test]
fn verification_rejects_default_sibling_for_its_own_domain() {
    let decode_domain = CommitmentDomainId::Wasm;
    let verification_domain = CommitmentDomainId::ExecutionAccounting;
    let verification_empty = empty_hashes(verification_domain);
    let mut bytes = Vec::with_capacity(66);
    bytes.extend_from_slice(&1u16.to_le_bytes());
    let mut bitmap = [0u8; 32];
    bitmap[0] = 0x80;
    bytes.extend_from_slice(&bitmap);
    bytes.extend_from_slice(verification_empty[1].as_bytes());

    let proof = SparseMerkleProofV1::decode(decode_domain, &bytes).unwrap();
    assert!(matches!(
        verify_proof(
            verification_domain,
            b"missing",
            None,
            &proof,
            verification_empty[0],
        ),
        Err(StateError::RedundantDefaultSibling(0))
    ));
}

#[test]
fn proof_verification_binds_domain_key_value_and_root() {
    let domain = CommitmentDomainId::Wasm;
    let (_, snapshot) = single_alpha_state();
    let proof =
        SparseMerkleProofV1::decode(domain, &decode_hex(SINGLE_ALPHA_MEMBERSHIP_HEX)).unwrap();

    verify_proof(domain, b"alpha", Some(b"beta"), &proof, snapshot.root).unwrap();
    assert!(
        verify_proof(
            CommitmentDomainId::ExecutionAccounting,
            b"alpha",
            Some(b"beta"),
            &proof,
            snapshot.root,
        )
        .is_err()
    );
    assert!(verify_proof(domain, b"omega", Some(b"beta"), &proof, snapshot.root).is_err());
    assert!(verify_proof(domain, b"alpha", Some(b"gamma"), &proof, snapshot.root).is_err());
    assert!(
        verify_proof(
            domain,
            b"alpha",
            Some(b"beta"),
            &proof,
            empty_hashes(domain)[0],
        )
        .is_err()
    );
}

#[test]
fn nonmembership_proof_rejects_wrong_context_and_tampering() {
    let domain = CommitmentDomainId::Wasm;
    let (_, snapshot) = triple_state();
    let literal = decode_hex(TRIPLE_MISSING_NONMEMBERSHIP_HEX);
    let proof = SparseMerkleProofV1::decode(domain, &literal).unwrap();

    assert!(verify_proof(domain, b"other", None, &proof, snapshot.root).is_err());
    assert!(verify_proof(domain, b"missing", Some(b""), &proof, snapshot.root).is_err());
    assert!(
        verify_proof(
            CommitmentDomainId::ExecutionAccounting,
            b"missing",
            None,
            &proof,
            snapshot.root,
        )
        .is_err()
    );
    assert!(verify_proof(domain, b"missing", None, &proof, empty_hashes(domain)[0],).is_err());

    let mut sibling_tampered = literal.clone();
    *sibling_tampered.last_mut().unwrap() ^= 1;
    let sibling_tampered = SparseMerkleProofV1::decode(domain, &sibling_tampered).unwrap();
    assert!(verify_proof(domain, b"missing", None, &sibling_tampered, snapshot.root,).is_err());

    let mut bitmap_tampered = literal;
    bitmap_tampered[2] = 0x50;
    let bitmap_tampered = SparseMerkleProofV1::decode(domain, &bitmap_tampered).unwrap();
    assert!(verify_proof(domain, b"missing", None, &bitmap_tampered, snapshot.root,).is_err());
}

#[test]
fn present_empty_value_survives_transition_and_proof() {
    let domain = CommitmentDomainId::Wasm;
    let mut source = MemorySource::default();
    let transition = apply_write_set(
        &source,
        empty_snapshot(domain),
        &write_set(domain, vec![StateWrite::put(b"empty".to_vec(), Vec::new())]),
    )
    .unwrap();
    let snapshot = DomainSnapshot {
        domain,
        root: transition.new_root,
    };
    source.absorb(&transition);

    let (value, proof) = prove(&source, snapshot, b"empty").unwrap();
    assert_eq!(value, Some(Vec::new()));
    verify_proof(domain, b"empty", Some(b""), &proof, snapshot.root).unwrap();
    assert!(verify_proof(domain, b"empty", None, &proof, snapshot.root).is_err());
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

#[test]
fn proof_construction_fails_closed_on_wrong_node_hash_and_depth() {
    let domain = CommitmentDomainId::Wasm;
    let empty = empty_hashes(domain);

    let requested = Hash256::from_bytes([0x11; 32]);
    let mut wrong_hash_source = MemorySource::default();
    wrong_hash_source.nodes.insert(
        requested,
        StateNode::Branch {
            depth: 0,
            left: empty[1],
            right: Hash256::from_bytes([0x22; 32]),
        },
    );
    assert!(matches!(
        prove(
            &wrong_hash_source,
            DomainSnapshot {
                domain,
                root: requested,
            },
            b"alpha",
        ),
        Err(StateError::NodeHashMismatch(hash)) if hash == requested
    ));

    let wrong_depth_node = StateNode::Branch {
        depth: 1,
        left: empty[2],
        right: Hash256::from_bytes([0x33; 32]),
    };
    let wrong_depth_root = wrong_depth_node.hash(domain).unwrap();
    let mut wrong_depth_source = MemorySource::default();
    wrong_depth_source
        .nodes
        .insert(wrong_depth_root, wrong_depth_node);
    assert!(matches!(
        prove(
            &wrong_depth_source,
            DomainSnapshot {
                domain,
                root: wrong_depth_root,
            },
            b"alpha",
        ),
        Err(StateError::NodeDepthMismatch {
            expected: 0,
            actual: 1,
        })
    ));
}

#[test]
fn proof_construction_fails_closed_on_missing_and_corrupt_value() {
    let (mut missing_source, snapshot) = single_alpha_state();
    let committed_value_hash = *missing_source.values.keys().next().unwrap();
    missing_source.values.remove(&committed_value_hash);
    assert!(matches!(
        prove(&missing_source, snapshot, b"alpha"),
        Err(StateError::MissingValue(hash)) if hash == committed_value_hash
    ));

    let (mut corrupt_source, snapshot) = single_alpha_state();
    corrupt_source
        .values
        .insert(committed_value_hash, b"tampered".to_vec());
    assert!(matches!(
        prove(&corrupt_source, snapshot, b"alpha"),
        Err(StateError::ValueHashMismatch(hash)) if hash == committed_value_hash
    ));
}

proptest! {
    #[test]
    fn arbitrary_proof_bytes_up_to_8500_never_panic(
        bytes in prop::collection::vec(any::<u8>(), 0..=8_500),
    ) {
        let _ = SparseMerkleProofV1::decode(CommitmentDomainId::Wasm, &bytes);
    }
}
