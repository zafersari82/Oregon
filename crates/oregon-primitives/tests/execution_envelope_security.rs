use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::execution_envelope::{
    AuthorizationProof, AuthorizationScheme, AuthorizationScope, ExecutionDomain,
    ExecutionEnvelopeError, ExecutionEnvelopeV1, ExecutionEnvelopeV1Parts, FeeCaps,
};

fn oregon_address(byte: u8) -> ExecutionAddress {
    ExecutionAddress::new(ExecutionAddressKind::Oregon, [byte; 32]).unwrap()
}

fn evm_address(byte: u8) -> ExecutionAddress {
    ExecutionAddress::from_evm([byte; 20])
}

fn schnorr(byte: u8, scope: AuthorizationScope) -> AuthorizationProof {
    AuthorizationProof::new(scope, AuthorizationScheme::OregonSchnorrV1, vec![byte; 96]).unwrap()
}

fn native_parts() -> ExecutionEnvelopeV1Parts {
    ExecutionEnvelopeV1Parts {
        chain_id: 42,
        execution_domain: ExecutionDomain::Native,
        valid_after_height: 1,
        valid_until_height: 500,
        principal: oregon_address(0x11),
        fee_payer: None,
        fee_caps: FeeCaps::new(50, 5, 10_000).unwrap(),
        authorizations: vec![schnorr(0x22, AuthorizationScope::Principal)],
        domain_payload: vec![0xaa, 0xbb],
        access_hints: Some(vec![0xcc]),
    }
}

#[test]
fn ethereum_source_authorization_requires_neutral_height_window() {
    let ecdsa = AuthorizationProof::new(
        AuthorizationScope::Principal,
        AuthorizationScheme::EthereumEcdsaV1,
        vec![0x55; 65],
    )
    .unwrap();

    for (valid_after_height, valid_until_height) in [(1, u64::MAX), (0, 500)] {
        let parts = ExecutionEnvelopeV1Parts {
            chain_id: 1,
            execution_domain: ExecutionDomain::Evm,
            valid_after_height,
            valid_until_height,
            principal: evm_address(0x44),
            fee_payer: None,
            fee_caps: FeeCaps::new(10, 1, 21_000).unwrap(),
            authorizations: vec![ecdsa.clone()],
            domain_payload: vec![],
            access_hints: None,
        };
        assert_eq!(
            ExecutionEnvelopeV1::new(parts),
            Err(ExecutionEnvelopeError::EthereumAuthorizationRequiresNeutralHeightWindow)
        );
    }
}

#[test]
fn native_signing_commitment_binds_all_authority_bearing_common_fields() {
    let base = ExecutionEnvelopeV1::new(native_parts()).unwrap();
    let base_hash = base.signing_hash();

    let mut changed = native_parts();
    changed.valid_after_height = 2;
    assert_ne!(
        ExecutionEnvelopeV1::new(changed).unwrap().signing_hash(),
        base_hash
    );

    let mut changed = native_parts();
    changed.valid_until_height = 501;
    assert_ne!(
        ExecutionEnvelopeV1::new(changed).unwrap().signing_hash(),
        base_hash
    );

    let mut changed = native_parts();
    changed.principal = oregon_address(0x12);
    assert_ne!(
        ExecutionEnvelopeV1::new(changed).unwrap().signing_hash(),
        base_hash
    );

    let mut changed = native_parts();
    changed.fee_caps = FeeCaps::new(51, 5, 10_000).unwrap();
    assert_ne!(
        ExecutionEnvelopeV1::new(changed).unwrap().signing_hash(),
        base_hash
    );

    let mut changed = native_parts();
    changed.fee_caps = FeeCaps::new(50, 6, 10_000).unwrap();
    assert_ne!(
        ExecutionEnvelopeV1::new(changed).unwrap().signing_hash(),
        base_hash
    );

    let mut changed = native_parts();
    changed.fee_caps = FeeCaps::new(50, 5, 10_001).unwrap();
    assert_ne!(
        ExecutionEnvelopeV1::new(changed).unwrap().signing_hash(),
        base_hash
    );

    let mut changed = native_parts();
    changed.domain_payload.push(0xdd);
    assert_ne!(
        ExecutionEnvelopeV1::new(changed).unwrap().signing_hash(),
        base_hash
    );

    let mut changed = native_parts();
    changed.access_hints = Some(vec![0xcd]);
    assert_ne!(
        ExecutionEnvelopeV1::new(changed).unwrap().signing_hash(),
        base_hash
    );

    let mut changed = native_parts();
    changed.authorizations = vec![
        AuthorizationProof::new(
            AuthorizationScope::Principal,
            AuthorizationScheme::OregonThresholdV1,
            vec![0x99],
        )
        .unwrap(),
    ];
    assert_ne!(
        ExecutionEnvelopeV1::new(changed).unwrap().signing_hash(),
        base_hash
    );
}
