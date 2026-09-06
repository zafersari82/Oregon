use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use oregon_primitives::Hash256;
use oregon_primitives::state_commitment::{
    CommitmentDomainId, CommitmentSchemeId, MAX_STATE_COMMITMENTS, StateCommitmentDescriptor,
    StateCommitmentSetV1,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct VectorFile {
    aggregate_version: u16,
    vectors: Vec<AggregateVector>,
}

#[derive(Debug, Deserialize)]
struct AggregateVector {
    name: String,
    descriptors: Vec<DescriptorVector>,
    canonical_hex: String,
    aggregate_root_hex: String,
}

#[derive(Debug, Deserialize)]
struct DescriptorVector {
    domain_id: u16,
    scheme_id: u16,
    root_hex: String,
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(text, 16).unwrap()
        })
        .collect()
}

fn vectors() -> VectorFile {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/vectors/state-commitments-v1.json");
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn descriptor(
    domain: CommitmentDomainId,
    scheme: CommitmentSchemeId,
    byte: u8,
) -> StateCommitmentDescriptor {
    StateCommitmentDescriptor::new(domain, scheme, Hash256::from_bytes([byte; 32]))
}

#[test]
fn commitment_ids_are_frozen_and_unknown_ids_fail_closed() {
    assert_eq!(u16::from(CommitmentDomainId::NativeUtxo), 0x0001);
    assert_eq!(u16::from(CommitmentDomainId::Evm), 0x0010);
    assert_eq!(u16::from(CommitmentDomainId::Wasm), 0x0011);
    assert_eq!(u16::from(CommitmentDomainId::ExecutionAccounting), 0x0020);
    assert_eq!(u16::from(CommitmentDomainId::ExecutionReceipts), 0x0030);
    assert_eq!(u16::from(CommitmentDomainId::AsyncOutbox), 0x0040);
    assert_eq!(u16::from(CommitmentDomainId::AsyncConsumed), 0x0041);
    assert_eq!(u16::from(CommitmentDomainId::FeeState), 0x0050);

    assert_eq!(u16::from(CommitmentSchemeId::OregonSmtV1), 0x0001);
    assert_eq!(u16::from(CommitmentSchemeId::EvmCommitmentV1), 0x0100);

    assert!(CommitmentDomainId::try_from(0xffff).is_err());
    assert!(CommitmentSchemeId::try_from(0xffff).is_err());
    assert_eq!(MAX_STATE_COMMITMENTS, 32);
}

#[test]
fn descriptor_is_exactly_36_bytes_and_rejects_non_exact_input() {
    let descriptor = descriptor(
        CommitmentDomainId::Wasm,
        CommitmentSchemeId::OregonSmtV1,
        0x11,
    );
    let encoded = descriptor.encode();
    assert_eq!(encoded.len(), 36);
    assert_eq!(
        StateCommitmentDescriptor::decode(&encoded).unwrap(),
        descriptor
    );

    for cut in 0..36 {
        assert!(StateCommitmentDescriptor::decode(&encoded[..cut]).is_err());
    }

    let mut trailing = encoded.to_vec();
    trailing.push(0);
    assert!(StateCommitmentDescriptor::decode(&trailing).is_err());

    let mut unknown_domain = encoded;
    unknown_domain[0..2].copy_from_slice(&0xffffu16.to_le_bytes());
    assert!(StateCommitmentDescriptor::decode(&unknown_domain).is_err());
}

#[test]
fn aggregate_literal_vectors_round_trip_exactly() {
    let vectors = vectors();
    assert_eq!(vectors.aggregate_version, 1);

    for vector in vectors.vectors {
        let canonical = decode_hex(&vector.canonical_hex);
        let decoded = StateCommitmentSetV1::decode(&canonical)
            .unwrap_or_else(|error| panic!("{} failed to decode: {error}", vector.name));
        assert_eq!(
            decoded.encode(),
            canonical,
            "{} canonical bytes",
            vector.name
        );
        assert_eq!(
            decoded.root(),
            Hash256::from_str(&vector.aggregate_root_hex).unwrap(),
            "{} aggregate root",
            vector.name
        );

        let expected: Vec<StateCommitmentDescriptor> = vector
            .descriptors
            .iter()
            .map(|item| {
                StateCommitmentDescriptor::new(
                    CommitmentDomainId::try_from(item.domain_id).unwrap(),
                    CommitmentSchemeId::try_from(item.scheme_id).unwrap(),
                    Hash256::from_str(&item.root_hex).unwrap(),
                )
            })
            .collect();
        assert_eq!(decoded.descriptors(), expected.as_slice());
    }
}

#[test]
fn aggregate_rejects_empty_malformed_oversized_unsorted_and_duplicate_domains() {
    assert!(StateCommitmentSetV1::new(vec![]).is_err());
    assert!(StateCommitmentSetV1::decode(&1u16.to_le_bytes()).is_err());

    let wasm = descriptor(
        CommitmentDomainId::Wasm,
        CommitmentSchemeId::OregonSmtV1,
        0x11,
    );
    let accounting = descriptor(
        CommitmentDomainId::ExecutionAccounting,
        CommitmentSchemeId::OregonSmtV1,
        0x22,
    );

    assert!(StateCommitmentSetV1::new(vec![accounting, wasm]).is_err());
    assert!(StateCommitmentSetV1::new(vec![wasm, wasm]).is_err());

    let mut malformed = vec![1, 0];
    malformed.extend_from_slice(&wasm.encode());
    malformed.push(0);
    assert!(StateCommitmentSetV1::decode(&malformed).is_err());

    let mut wrong_version = vec![2, 0];
    wrong_version.extend_from_slice(&wasm.encode());
    assert!(StateCommitmentSetV1::decode(&wrong_version).is_err());

    let mut oversized = vec![1, 0];
    for _ in 0..=MAX_STATE_COMMITMENTS {
        oversized.extend_from_slice(&wasm.encode());
    }
    assert!(StateCommitmentSetV1::decode(&oversized).is_err());
}

#[test]
fn aggregate_root_binds_domain_scheme_and_child_root() {
    let base = StateCommitmentSetV1::new(vec![descriptor(
        CommitmentDomainId::Wasm,
        CommitmentSchemeId::OregonSmtV1,
        0x11,
    )])
    .unwrap();

    let changed_domain = StateCommitmentSetV1::new(vec![descriptor(
        CommitmentDomainId::ExecutionAccounting,
        CommitmentSchemeId::OregonSmtV1,
        0x11,
    )])
    .unwrap();
    let changed_scheme = StateCommitmentSetV1::new(vec![descriptor(
        CommitmentDomainId::Wasm,
        CommitmentSchemeId::EvmCommitmentV1,
        0x11,
    )])
    .unwrap();
    let changed_root = StateCommitmentSetV1::new(vec![descriptor(
        CommitmentDomainId::Wasm,
        CommitmentSchemeId::OregonSmtV1,
        0x12,
    )])
    .unwrap();

    assert_ne!(base.root(), changed_domain.root());
    assert_ne!(base.root(), changed_scheme.root());
    assert_ne!(base.root(), changed_root.root());
}
