use std::collections::HashSet;

use oregon_primitives::execution_address::{
    ExecutionAddress, ExecutionAddressError, ExecutionAddressKind,
};
use proptest::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct Vector {
    name: String,
    kind: u8,
    payload_hex: String,
    canonical_hex: String,
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn canonical_vectors_preserve_kind_and_every_payload_byte() {
    let vectors: Vec<Vector> = serde_json::from_str(include_str!(
        "../../../tests/vectors/execution-address-v1.json"
    ))
    .unwrap();
    for vector in vectors {
        let bytes = decode_hex(&vector.canonical_hex);
        let payload: [u8; 32] = decode_hex(&vector.payload_hex).try_into().unwrap();
        let kind = ExecutionAddressKind::try_from(vector.kind).unwrap();
        let address = ExecutionAddress::new(kind, payload).unwrap();
        assert_eq!(address.to_bytes().as_slice(), bytes, "{}", vector.name);
        assert_eq!(ExecutionAddress::from_slice(&bytes), Ok(address));
        assert_eq!(address.kind(), kind);
        assert_eq!(address.payload(), &payload);
    }
}

#[test]
fn unknown_kinds_are_rejected_instead_of_becoming_another_namespace() {
    for tag in 0u8..=255 {
        if (1..=4).contains(&tag) {
            continue;
        }
        let mut bytes = [0; 33];
        bytes[0] = tag;
        assert_eq!(
            ExecutionAddressKind::try_from(tag),
            Err(ExecutionAddressError::UnknownKind(tag))
        );
        assert_eq!(
            ExecutionAddress::from_slice(&bytes),
            Err(ExecutionAddressError::UnknownKind(tag))
        );
    }
}

#[test]
fn decoder_rejects_every_truncated_length_and_trailing_bytes() {
    for length in (0..33).chain([34, 64, 4096]) {
        let mut bytes = vec![0; length];
        if let Some(tag) = bytes.first_mut() {
            *tag = 2;
        }
        assert_eq!(
            ExecutionAddress::from_slice(&bytes),
            Err(ExecutionAddressError::InvalidLength(length))
        );
    }
}

#[test]
fn nonzero_evm_padding_cannot_create_aliases() {
    for index in 0..12 {
        for value in [1, 128, 255] {
            let mut payload = [0; 32];
            payload[index] = value;
            assert_eq!(
                ExecutionAddress::new(ExecutionAddressKind::Evm, payload),
                Err(ExecutionAddressError::NonCanonicalEvmPadding)
            );
            let mut bytes = [0; 33];
            bytes[0] = 1;
            bytes[1..].copy_from_slice(&payload);
            assert_eq!(
                ExecutionAddress::from_slice(&bytes),
                Err(ExecutionAddressError::NonCanonicalEvmPadding)
            );
        }
    }
}

#[test]
fn identical_payloads_in_different_namespaces_remain_distinct() {
    let mut identities = HashSet::new();
    for kind in [
        ExecutionAddressKind::Evm,
        ExecutionAddressKind::Wasm,
        ExecutionAddressKind::Oregon,
        ExecutionAddressKind::System,
    ] {
        let address = ExecutionAddress::new(kind, [0; 32]).unwrap();
        assert!(identities.insert(address));
        if kind != ExecutionAddressKind::Evm {
            assert_eq!(address.evm_address(), None);
        }
    }
    assert_eq!(identities.len(), 4);
}

#[test]
fn evm_mapping_preserves_the_external_twenty_byte_address() {
    let external = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
        0xee, 0xff, 0x01, 0x23, 0x45, 0x67,
    ];
    let expected = decode_hex("0100000000000000000000000000112233445566778899aabbccddeeff01234567");
    let address = ExecutionAddress::from_evm(external);
    assert_eq!(address.to_bytes().as_slice(), expected);
    assert_eq!(address.evm_address(), Some(external));
}

proptest! {
    #[test]
    fn arbitrary_evm_addresses_preserve_external_identity(external in any::<[u8; 20]>()) {
        let address = ExecutionAddress::from_evm(external);
        prop_assert_eq!(address.kind(), ExecutionAddressKind::Evm);
        prop_assert_eq!(address.evm_address(), Some(external));
        prop_assert_eq!(&address.to_bytes()[1..13], &[0; 12]);
        prop_assert_eq!(ExecutionAddress::from_slice(&address.to_bytes()), Ok(address));
    }

    #[test]
    fn non_evm_namespaces_preserve_all_32_bytes(tag in 2u8..=4, payload in any::<[u8; 32]>()) {
        let kind = ExecutionAddressKind::try_from(tag).unwrap();
        let address = ExecutionAddress::new(kind, payload).unwrap();
        prop_assert_eq!(address.payload(), &payload);
        prop_assert_eq!(&address.to_bytes()[1..], &payload);
        prop_assert_eq!(address.evm_address(), None);
        prop_assert_eq!(ExecutionAddress::from_slice(&address.to_bytes()), Ok(address));
    }
}
