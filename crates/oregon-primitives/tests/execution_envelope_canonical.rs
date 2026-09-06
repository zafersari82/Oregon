use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::execution_envelope::{
    AuthorizationProof, AuthorizationScheme, AuthorizationScope, ExecutionDomain,
    ExecutionEnvelopeV1, ExecutionEnvelopeV1Parts, FeeCaps,
};

fn oregon_address(byte: u8) -> ExecutionAddress {
    ExecutionAddress::new(ExecutionAddressKind::Oregon, [byte; 32]).unwrap()
}

fn schnorr(byte: u8, scope: AuthorizationScope) -> AuthorizationProof {
    AuthorizationProof::new(scope, AuthorizationScheme::OregonSchnorrV1, vec![byte; 96]).unwrap()
}

fn distinct_fee_payer_envelope(fee_payer_byte: u8) -> ExecutionEnvelopeV1 {
    ExecutionEnvelopeV1::new(ExecutionEnvelopeV1Parts {
        chain_id: 42,
        execution_domain: ExecutionDomain::Native,
        valid_after_height: 7,
        valid_until_height: 700,
        principal: oregon_address(0x11),
        fee_payer: Some(oregon_address(fee_payer_byte)),
        fee_caps: FeeCaps::new(50, 5, 10_000).unwrap(),
        authorizations: vec![
            schnorr(0x22, AuthorizationScope::Principal),
            schnorr(0x33, AuthorizationScope::FeePayer),
        ],
        domain_payload: vec![0xaa, 0xbb, 0xcc],
        access_hints: Some(vec![0xdd, 0xee]),
    })
    .unwrap()
}

#[test]
fn every_variable_length_position_rejects_nonminimal_varint() {
    let envelope = distinct_fee_payer_envelope(0x12);
    let bytes = envelope.encode();

    // Layout for this exact canonical V1 fixture:
    // auth_count, proof_len #1, proof_len #2, payload_len, access_hint_len.
    let positions_and_values = [
        (118usize, 2u8),
        (122usize, 96u8),
        (222usize, 96u8),
        (319usize, 3u8),
        (324usize, 2u8),
    ];

    assert_eq!(bytes.len(), 327);
    for (offset, value) in positions_and_values {
        assert_eq!(bytes[offset], value, "fixture drift at offset {offset}");
        let mut nonminimal = bytes.clone();
        nonminimal.splice(offset..=offset, [0xfd, value, 0x00]);
        assert!(
            ExecutionEnvelopeV1::decode(&nonminimal).is_err(),
            "non-minimal varint accepted at offset {offset}"
        );
    }
}

#[test]
fn signing_bytes_bind_fee_payer_and_authorization_scopes() {
    let first = distinct_fee_payer_envelope(0x12);
    let second = distinct_fee_payer_envelope(0x13);

    assert_ne!(first.signing_hash(), second.signing_hash());

    let signing = first.signing_bytes();
    assert_eq!(
        &signing[118..125],
        &[0x02, 0x01, 0x01, 0x00, 0x02, 0x01, 0x00],
        "authorization count/scope/scheme bytes must be committed canonically"
    );
}
