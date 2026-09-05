use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use oregon_contract_state::{
    DomainSnapshot, StateWrite, StateWriteSet, apply_write_set, branch_hash, empty_hashes,
    leaf_hash, path_bit, path_key, value_hash,
};
use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::CommitmentDomainId;
use serde::Deserialize;

mod support;
use support::MemorySource;

#[derive(Debug, Deserialize)]
struct VectorFile {
    version: u16,
    domains: Vec<DomainVector>,
}

#[derive(Debug, Deserialize)]
struct DomainVector {
    name: String,
    domain_id: u16,
    empty_root_hex: String,
    present_empty_value_hash_hex: String,
    entries: Vec<EntryVector>,
    state_roots: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct EntryVector {
    key_hex: String,
    value_hex: String,
    path_key_hex: String,
    value_hash_hex: String,
    one_leaf_root_hex: Option<String>,
}

fn decode_hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn vectors() -> VectorFile {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/vectors/contract-state-smt-v1.json");
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn one_leaf_root(domain: CommitmentDomainId, path: Hash256, value: Hash256) -> Hash256 {
    let empty = empty_hashes(domain);
    let mut current = leaf_hash(domain, path, value);
    for depth in (0..256).rev() {
        let sibling = empty[depth + 1];
        current = if path_bit(path, depth).unwrap() {
            branch_hash(domain, depth as u16, sibling, current).unwrap()
        } else {
            branch_hash(domain, depth as u16, current, sibling).unwrap()
        };
    }
    current
}

#[test]
fn literal_vectors_pin_path_value_empty_and_one_leaf_roots() {
    let vectors = vectors();
    assert_eq!(vectors.version, 1);

    for domain_vector in vectors.domains {
        let domain = CommitmentDomainId::try_from(domain_vector.domain_id).unwrap();
        let empty = empty_hashes(domain);
        assert_eq!(
            empty[0],
            Hash256::from_str(&domain_vector.empty_root_hex).unwrap(),
            "{} empty root",
            domain_vector.name
        );
        assert_eq!(
            value_hash(domain, &[]).unwrap(),
            Hash256::from_str(&domain_vector.present_empty_value_hash_hex).unwrap(),
            "{} present-empty value hash",
            domain_vector.name
        );

        for entry in domain_vector.entries {
            let key = decode_hex(&entry.key_hex);
            let value = decode_hex(&entry.value_hex);
            let path = path_key(domain, &key).unwrap();
            let value_commitment = value_hash(domain, &value).unwrap();
            assert_eq!(
                path,
                Hash256::from_str(&entry.path_key_hex).unwrap(),
                "{} path key",
                domain_vector.name
            );
            assert_eq!(
                value_commitment,
                Hash256::from_str(&entry.value_hash_hex).unwrap(),
                "{} value hash",
                domain_vector.name
            );

            if let Some(expected) = entry.one_leaf_root_hex {
                assert_eq!(
                    one_leaf_root(domain, path, value_commitment),
                    Hash256::from_str(&expected).unwrap(),
                    "{} one-leaf root",
                    domain_vector.name
                );
            }
        }
    }
}

#[test]
fn path_bits_are_msb_first_at_frozen_boundaries() {
    let mut bytes = [0u8; 32];
    bytes[0] = 0x81;
    bytes[1] = 0x80;
    bytes[31] = 0x01;
    let path = Hash256::from_bytes(bytes);

    assert!(path_bit(path, 0).unwrap());
    assert!(!path_bit(path, 1).unwrap());
    assert!(path_bit(path, 7).unwrap());
    assert!(path_bit(path, 8).unwrap());
    assert!(path_bit(path, 255).unwrap());
    assert!(path_bit(path, 256).is_err());
}

#[test]
fn present_empty_value_is_not_absence() {
    let domain = CommitmentDomainId::Wasm;
    let key = b"alpha";
    let path = path_key(domain, key).unwrap();
    let present_empty = value_hash(domain, b"").unwrap();
    let root = one_leaf_root(domain, path, present_empty);
    assert_ne!(root, empty_hashes(domain)[0]);
}

#[test]
fn identical_raw_key_and_value_are_domain_separated() {
    let key = b"alpha";
    let value = b"beta";
    let wasm_path = path_key(CommitmentDomainId::Wasm, key).unwrap();
    let accounting_path = path_key(CommitmentDomainId::ExecutionAccounting, key).unwrap();
    let wasm_value = value_hash(CommitmentDomainId::Wasm, value).unwrap();
    let accounting_value = value_hash(CommitmentDomainId::ExecutionAccounting, value).unwrap();

    assert_ne!(wasm_path, accounting_path);
    assert_ne!(wasm_value, accounting_value);
    assert_ne!(
        one_leaf_root(CommitmentDomainId::Wasm, wasm_path, wasm_value),
        one_leaf_root(
            CommitmentDomainId::ExecutionAccounting,
            accounting_path,
            accounting_value,
        )
    );
}

#[test]
fn execution_accounting_transition_vectors_cover_update_delete_and_shared_prefix() {
    let vector = vectors()
        .domains
        .into_iter()
        .find(|vector| vector.domain_id == u16::from(CommitmentDomainId::ExecutionAccounting))
        .unwrap();
    let domain = CommitmentDomainId::ExecutionAccounting;
    let empty_root = Hash256::from_str(&vector.empty_root_hex).unwrap();
    let mut source = MemorySource::default();

    let alpha = apply_write_set(
        &source,
        DomainSnapshot {
            domain,
            root: empty_root,
        },
        &StateWriteSet::new(
            domain,
            vec![StateWrite::put(b"alpha".to_vec(), b"beta".to_vec())],
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        alpha.new_root,
        Hash256::from_str(vector.entries[0].one_leaf_root_hex.as_deref().unwrap(),).unwrap()
    );
    source.absorb(&alpha);
    let alpha_snapshot = DomainSnapshot {
        domain,
        root: alpha.new_root,
    };

    let updated = apply_write_set(
        &source,
        alpha_snapshot,
        &StateWriteSet::new(
            domain,
            vec![StateWrite::put(b"alpha".to_vec(), b"gamma".to_vec())],
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        updated.new_root,
        Hash256::from_str(&vector.state_roots["alpha_gamma"]).unwrap()
    );

    let deleted = apply_write_set(
        &source,
        alpha_snapshot,
        &StateWriteSet::new(domain, vec![StateWrite::delete(b"alpha".to_vec())]).unwrap(),
    )
    .unwrap();
    assert_eq!(deleted.new_root, empty_root);

    let two_leaf = apply_write_set(
        &MemorySource::default(),
        DomainSnapshot {
            domain,
            root: empty_root,
        },
        &StateWriteSet::new(
            domain,
            vec![
                StateWrite::put(b"alpha".to_vec(), b"beta".to_vec()),
                StateWrite::put(b"omega".to_vec(), b"zeta".to_vec()),
            ],
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        two_leaf.new_root,
        Hash256::from_str(&vector.state_roots["alpha_beta_omega_zeta"]).unwrap()
    );

    let long_prefix = apply_write_set(
        &MemorySource::default(),
        DomainSnapshot {
            domain,
            root: empty_root,
        },
        &StateWriteSet::new(
            domain,
            vec![
                StateWrite::put(b"alpha".to_vec(), b"beta".to_vec()),
                StateWrite::put(b"k62521".to_vec(), b"theta".to_vec()),
            ],
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        long_prefix.new_root,
        Hash256::from_str(&vector.state_roots["alpha_beta_k62521_theta"]).unwrap()
    );
}
