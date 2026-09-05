use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::execution_envelope::{
    AuthorizationProof, AuthorizationScheme, AuthorizationScope, ExecutionDomain,
    ExecutionEnvelopeError, ExecutionEnvelopeV1, ExecutionEnvelopeV1Parts, FeeCaps,
    MAX_ACCESS_HINT_BYTES, MAX_AUTH_PROOF_BYTES, MAX_DOMAIN_PAYLOAD_BYTES, MAX_ENVELOPE_BYTES,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Vector {
    name: String,
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

fn oregon_address(byte: u8) -> ExecutionAddress {
    ExecutionAddress::new(ExecutionAddressKind::Oregon, [byte; 32]).unwrap()
}

fn evm_address(byte: u8) -> ExecutionAddress {
    ExecutionAddress::from_evm([byte; 20])
}

fn schnorr(byte: u8, scope: AuthorizationScope) -> AuthorizationProof {
    let mut proof = vec![byte; 96];
    proof[..32].fill(byte.wrapping_add(1));
    AuthorizationProof::new(scope, AuthorizationScheme::OregonSchnorrV1, proof).unwrap()
}

fn native_parts(proof_byte: u8) -> ExecutionEnvelopeV1Parts {
    ExecutionEnvelopeV1Parts {
        chain_id: 42,
        execution_domain: ExecutionDomain::Native,
        valid_after_height: 0,
        valid_until_height: u64::MAX,
        principal: oregon_address(0x11),
        fee_payer: None,
        fee_caps: FeeCaps::new(5, 2, 1_000).unwrap(),
        authorizations: vec![schnorr(proof_byte, AuthorizationScope::Principal)],
        domain_payload: vec![0xaa, 0xbb, 0xcc],
        access_hints: None,
    }
}

#[test]
fn literal_vectors_round_trip_exactly() {
    let vectors: Vec<Vector> = serde_json::from_str(include_str!(
        "../../../tests/vectors/execution-envelope-v1.json"
    ))
    .unwrap();

    for vector in vectors {
        let bytes = decode_hex(&vector.canonical_hex);
        let decoded = ExecutionEnvelopeV1::decode(&bytes).unwrap_or_else(|error| {
            panic!("{} failed to decode: {error:?}", vector.name)
        });
        assert_eq!(decoded.encode(), bytes, "{}", vector.name);
    }
}

#[test]
fn discriminants_are_frozen_and_unknown_values_fail_closed() {
    for (tag, domain) in [
        (0x10, ExecutionDomain::Native),
        (0x11, ExecutionDomain::Evm),
        (0x12, ExecutionDomain::Wasm),
        (0x13, ExecutionDomain::System),
    ] {
        assert_eq!(ExecutionDomain::try_from(tag), Ok(domain));
    }
    for tag in 0u8..=255 {
        if (0x10..=0x13).contains(&tag) {
            continue;
        }
        assert_eq!(
            ExecutionDomain::try_from(tag),
            Err(ExecutionEnvelopeError::UnknownDomain(tag))
        );
    }

    assert_eq!(
        AuthorizationScope::try_from(0x01),
        Ok(AuthorizationScope::Principal)
    );
    assert_eq!(
        AuthorizationScope::try_from(0x02),
        Ok(AuthorizationScope::FeePayer)
    );
    assert_eq!(
        AuthorizationScope::try_from(0x03),
        Err(ExecutionEnvelopeError::UnknownAuthorizationScope(0x03))
    );

    assert_eq!(
        AuthorizationScheme::try_from(0x0001),
        Ok(AuthorizationScheme::OregonSchnorrV1)
    );
    assert_eq!(
        AuthorizationScheme::try_from(0x0002),
        Ok(AuthorizationScheme::EthereumEcdsaV1)
    );
    assert_eq!(
        AuthorizationScheme::try_from(0x0003),
        Ok(AuthorizationScheme::OregonThresholdV1)
    );
    assert_eq!(
        AuthorizationScheme::try_from(0x0004),
        Err(ExecutionEnvelopeError::UnknownAuthorizationScheme(0x0004))
    );
}

#[test]
fn fee_caps_reject_noncanonical_values() {
    assert_eq!(
        FeeCaps::new(10, 11, 1),
        Err(ExecutionEnvelopeError::PriorityFeeExceedsMaxFee)
    );
    assert_eq!(
        FeeCaps::new(10, 1, 0),
        Err(ExecutionEnvelopeError::ZeroMaxWeight)
    );
    assert!(FeeCaps::new(10, 10, 1).is_ok());
}

#[test]
fn authorization_outer_lengths_are_exactly_bounded() {
    assert!(AuthorizationProof::new(
        AuthorizationScope::Principal,
        AuthorizationScheme::OregonSchnorrV1,
        vec![0; 96]
    )
    .is_ok());
    assert_eq!(
        AuthorizationProof::new(
            AuthorizationScope::Principal,
            AuthorizationScheme::OregonSchnorrV1,
            vec![0; 95]
        ),
        Err(ExecutionEnvelopeError::InvalidAuthorizationProofLength)
    );
    assert!(AuthorizationProof::new(
        AuthorizationScope::Principal,
        AuthorizationScheme::EthereumEcdsaV1,
        vec![0; 65]
    )
    .is_ok());
    assert_eq!(
        AuthorizationProof::new(
            AuthorizationScope::Principal,
            AuthorizationScheme::EthereumEcdsaV1,
            vec![0; 64]
        ),
        Err(ExecutionEnvelopeError::InvalidAuthorizationProofLength)
    );
    assert!(AuthorizationProof::new(
        AuthorizationScope::Principal,
        AuthorizationScheme::OregonThresholdV1,
        vec![0; 1]
    )
    .is_ok());
    assert!(AuthorizationProof::new(
        AuthorizationScope::Principal,
        AuthorizationScheme::OregonThresholdV1,
        vec![0; MAX_AUTH_PROOF_BYTES]
    )
    .is_ok());
    assert_eq!(
        AuthorizationProof::new(
            AuthorizationScope::Principal,
            AuthorizationScheme::OregonThresholdV1,
            vec![]
        ),
        Err(ExecutionEnvelopeError::InvalidAuthorizationProofLength)
    );
    assert_eq!(
        AuthorizationProof::new(
            AuthorizationScope::Principal,
            AuthorizationScheme::OregonThresholdV1,
            vec![0; MAX_AUTH_PROOF_BYTES + 1]
        ),
        Err(ExecutionEnvelopeError::InvalidAuthorizationProofLength)
    );
}

#[test]
fn fee_payer_presence_and_scope_rules_are_canonical() {
    let principal = oregon_address(0x11);
    let fee_payer = oregon_address(0x12);

    let mut equal_payer = native_parts(0x20);
    equal_payer.fee_payer = Some(principal);
    equal_payer.authorizations.push(schnorr(0x30, AuthorizationScope::FeePayer));
    assert_eq!(
        ExecutionEnvelopeV1::new(equal_payer),
        Err(ExecutionEnvelopeError::FeePayerEqualsPrincipal)
    );

    let mut missing_payer_auth = native_parts(0x20);
    missing_payer_auth.fee_payer = Some(fee_payer);
    assert_eq!(
        ExecutionEnvelopeV1::new(missing_payer_auth),
        Err(ExecutionEnvelopeError::MissingFeePayerAuthorization)
    );

    let mut unexpected_payer_auth = native_parts(0x20);
    unexpected_payer_auth
        .authorizations
        .push(schnorr(0x30, AuthorizationScope::FeePayer));
    assert_eq!(
        ExecutionEnvelopeV1::new(unexpected_payer_auth),
        Err(ExecutionEnvelopeError::UnexpectedFeePayerAuthorization)
    );

    let mut valid = native_parts(0x20);
    valid.fee_payer = Some(fee_payer);
    valid.authorizations.push(schnorr(0x30, AuthorizationScope::FeePayer));
    assert!(ExecutionEnvelopeV1::new(valid).is_ok());
}

#[test]
fn height_window_and_access_hint_rules_are_canonical() {
    let mut invalid_window = native_parts(0x20);
    invalid_window.valid_after_height = 10;
    invalid_window.valid_until_height = 9;
    assert_eq!(
        ExecutionEnvelopeV1::new(invalid_window),
        Err(ExecutionEnvelopeError::InvalidHeightWindow)
    );

    let mut empty_hints = native_parts(0x20);
    empty_hints.access_hints = Some(vec![]);
    assert_eq!(
        ExecutionEnvelopeV1::new(empty_hints),
        Err(ExecutionEnvelopeError::EmptyAccessHints)
    );

    let mut max_hints = native_parts(0x20);
    max_hints.access_hints = Some(vec![0; MAX_ACCESS_HINT_BYTES]);
    assert!(ExecutionEnvelopeV1::new(max_hints).is_ok());

    let mut too_many_hints = native_parts(0x20);
    too_many_hints.access_hints = Some(vec![0; MAX_ACCESS_HINT_BYTES + 1]);
    assert_eq!(
        ExecutionEnvelopeV1::new(too_many_hints),
        Err(ExecutionEnvelopeError::AccessHintsTooLarge)
    );
}

#[test]
fn domain_payload_limit_is_enforced_before_wire_use() {
    let mut max_payload = native_parts(0x20);
    max_payload.domain_payload = vec![0; MAX_DOMAIN_PAYLOAD_BYTES];
    assert!(ExecutionEnvelopeV1::new(max_payload).is_ok());

    let mut too_large = native_parts(0x20);
    too_large.domain_payload = vec![0; MAX_DOMAIN_PAYLOAD_BYTES + 1];
    assert_eq!(
        ExecutionEnvelopeV1::new(too_large),
        Err(ExecutionEnvelopeError::DomainPayloadTooLarge)
    );
}

#[test]
fn ethereum_authorization_is_evm_only_and_uses_neutral_height_window() {
    let ecdsa = AuthorizationProof::new(
        AuthorizationScope::Principal,
        AuthorizationScheme::EthereumEcdsaV1,
        vec![0x55; 65],
    )
    .unwrap();

    let native = ExecutionEnvelopeV1Parts {
        authorizations: vec![ecdsa.clone()],
        ..native_parts(0x20)
    };
    assert_eq!(
        ExecutionEnvelopeV1::new(native),
        Err(ExecutionEnvelopeError::EthereumAuthorizationOutsideEvm)
    );

    let evm = ExecutionEnvelopeV1Parts {
        chain_id: 1,
        execution_domain: ExecutionDomain::Evm,
        valid_after_height: 0,
        valid_until_height: u64::MAX,
        principal: evm_address(0x44),
        fee_payer: None,
        fee_caps: FeeCaps::new(10, 1, 21_000).unwrap(),
        authorizations: vec![ecdsa],
        domain_payload: vec![],
        access_hints: Some(vec![0x66, 0x77]),
    };
    assert!(ExecutionEnvelopeV1::new(evm).is_ok());
}

#[test]
fn proof_bytes_are_excluded_from_native_signing_hash_but_included_in_txid() {
    let first = ExecutionEnvelopeV1::new(native_parts(0x20)).unwrap();
    let second = ExecutionEnvelopeV1::new(native_parts(0x21)).unwrap();

    assert_eq!(first.signing_bytes(), second.signing_bytes());
    assert_eq!(first.signing_hash(), second.signing_hash());
    assert_ne!(first.txid(), second.txid());
}

#[test]
fn chain_and_domain_are_bound_into_native_signing_commitment() {
    let base = ExecutionEnvelopeV1::new(native_parts(0x20)).unwrap();

    let mut changed_chain = native_parts(0x20);
    changed_chain.chain_id = 43;
    let changed_chain = ExecutionEnvelopeV1::new(changed_chain).unwrap();
    assert_ne!(base.signing_hash(), changed_chain.signing_hash());

    let mut changed_domain = native_parts(0x20);
    changed_domain.execution_domain = ExecutionDomain::Wasm;
    let changed_domain = ExecutionEnvelopeV1::new(changed_domain).unwrap();
    assert_ne!(base.signing_hash(), changed_domain.signing_hash());
}

#[test]
fn decoder_rejects_nonminimal_count_varint_truncation_and_trailing_bytes() {
    let vector: Vec<Vector> = serde_json::from_str(include_str!(
        "../../../tests/vectors/execution-envelope-v1.json"
    ))
    .unwrap();
    let bytes = decode_hex(&vector[0].canonical_hex);

    for length in 0..bytes.len() {
        let truncated = &bytes[..length];
        assert!(ExecutionEnvelopeV1::decode(truncated).is_err(), "length={length}");
    }

    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(ExecutionEnvelopeV1::decode(&trailing).is_err());

    // The principal-only vector's authorization count is byte 85 and equals 1.
    assert_eq!(bytes[85], 1);
    let mut nonminimal = Vec::with_capacity(bytes.len() + 2);
    nonminimal.extend_from_slice(&bytes[..85]);
    nonminimal.extend_from_slice(&[0xfd, 0x01, 0x00]);
    nonminimal.extend_from_slice(&bytes[86..]);
    assert!(ExecutionEnvelopeV1::decode(&nonminimal).is_err());
}

#[test]
fn decoder_rejects_noncanonical_option_flags_and_total_size_first() {
    let vectors: Vec<Vector> = serde_json::from_str(include_str!(
        "../../../tests/vectors/execution-envelope-v1.json"
    ))
    .unwrap();
    let mut bytes = decode_hex(&vectors[0].canonical_hex);

    // Fee-payer option flag follows the fixed 33-byte principal at offset 60.
    assert_eq!(bytes[60], 0);
    bytes[60] = 2;
    assert_eq!(
        ExecutionEnvelopeV1::decode(&bytes),
        Err(ExecutionEnvelopeError::InvalidOptionFlag(2))
    );

    assert_eq!(
        ExecutionEnvelopeV1::decode(&vec![0; MAX_ENVELOPE_BYTES + 1]),
        Err(ExecutionEnvelopeError::EnvelopeTooLarge)
    );
}
