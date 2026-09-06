use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::execution_envelope::ExecutionDomain;
use oregon_primitives::Hash256;
use oregon_runtime::{
    MAX_RUNTIME_CALL_DEPTH, MAX_RUNTIME_CALL_INPUT_BYTES, MAX_RUNTIME_RETURN_DATA_BYTES,
    RuntimeAbiError, RuntimeBackendV1, RuntimeCallContextV1, RuntimeCallContextV1Parts,
    RuntimeCallResultV1, RuntimeCallSpecV1, RuntimeHostV1, RuntimeTrapCodeV1,
};

fn evm(byte: u8) -> ExecutionAddress {
    ExecutionAddress::from_evm([byte; 20])
}

fn wasm(byte: u8) -> ExecutionAddress {
    ExecutionAddress::new(ExecutionAddressKind::Wasm, [byte; 32]).unwrap()
}

fn oregon(byte: u8) -> ExecutionAddress {
    ExecutionAddress::new(ExecutionAddressKind::Oregon, [byte; 32]).unwrap()
}

fn system(byte: u8) -> ExecutionAddress {
    ExecutionAddress::new(ExecutionAddressKind::System, [byte; 32]).unwrap()
}

fn context_parts(target: ExecutionAddress, execution_domain: ExecutionDomain) -> RuntimeCallContextV1Parts {
    RuntimeCallContextV1Parts {
        chain_id: 42,
        height: 9001,
        parent_block_hash: Hash256::from_bytes([0x11; 32]),
        txid: Hash256::from_bytes([0x22; 32]),
        principal: oregon(0x33),
        caller: wasm(0x44),
        target,
        execution_domain,
        depth: 1,
        read_only: true,
        transferred_value: 7,
    }
}

#[test]
fn call_input_exact_limit_is_accepted() {
    let call = RuntimeCallSpecV1::new(
        evm(0x01),
        9,
        false,
        vec![0u8; MAX_RUNTIME_CALL_INPUT_BYTES],
    )
    .unwrap();
    assert_eq!(call.input().len(), MAX_RUNTIME_CALL_INPUT_BYTES);
    assert_eq!(call.value(), 9);
    assert!(!call.read_only());
}

#[test]
fn call_input_one_over_is_rejected_before_retention() {
    let input = vec![0u8; MAX_RUNTIME_CALL_INPUT_BYTES + 1];
    let err = RuntimeCallSpecV1::new(evm(0x01), 0, false, input).unwrap_err();
    assert_eq!(err, RuntimeAbiError::CallInputTooLarge);
}

#[test]
fn standard_runtime_calls_target_only_evm_or_wasm() {
    assert!(RuntimeCallSpecV1::new(evm(0x01), 0, false, Vec::new()).is_ok());
    assert!(RuntimeCallSpecV1::new(wasm(0x02), 0, false, Vec::new()).is_ok());
    assert_eq!(
        RuntimeCallSpecV1::new(oregon(0x03), 0, false, Vec::new()).unwrap_err(),
        RuntimeAbiError::InvalidCallTarget,
    );
    assert_eq!(
        RuntimeCallSpecV1::new(system(0x04), 0, false, Vec::new()).unwrap_err(),
        RuntimeAbiError::InvalidCallTarget,
    );
}

#[test]
fn return_data_exact_limit_is_accepted_and_one_over_is_rejected() {
    let success = RuntimeCallResultV1::success(vec![0u8; MAX_RUNTIME_RETURN_DATA_BYTES]).unwrap();
    assert_eq!(success.return_data().unwrap().len(), MAX_RUNTIME_RETURN_DATA_BYTES);

    let revert = RuntimeCallResultV1::revert(vec![0u8; MAX_RUNTIME_RETURN_DATA_BYTES]).unwrap();
    assert_eq!(revert.return_data().unwrap().len(), MAX_RUNTIME_RETURN_DATA_BYTES);

    assert_eq!(
        RuntimeCallResultV1::success(vec![0u8; MAX_RUNTIME_RETURN_DATA_BYTES + 1]).unwrap_err(),
        RuntimeAbiError::ReturnDataTooLarge,
    );
    assert_eq!(
        RuntimeCallResultV1::revert(vec![0u8; MAX_RUNTIME_RETURN_DATA_BYTES + 1]).unwrap_err(),
        RuntimeAbiError::ReturnDataTooLarge,
    );
}

#[test]
fn direct_backend_result_still_requires_boundary_validation() {
    let result = RuntimeCallResultV1::Success(vec![0u8; MAX_RUNTIME_RETURN_DATA_BYTES + 1]);
    assert_eq!(result.validate(), Err(RuntimeAbiError::ReturnDataTooLarge));
}

#[test]
fn trap_codes_have_exact_v1_discriminants_and_fail_closed() {
    let expected = [
        RuntimeTrapCodeV1::BackendDeterministic,
        RuntimeTrapCodeV1::InvalidCallTarget,
        RuntimeTrapCodeV1::StateAccessDenied,
        RuntimeTrapCodeV1::ReadOnlyViolation,
        RuntimeTrapCodeV1::CallDepthExceeded,
        RuntimeTrapCodeV1::CallInputTooLarge,
        RuntimeTrapCodeV1::ReturnDataTooLarge,
        RuntimeTrapCodeV1::EventLimitExceeded,
        RuntimeTrapCodeV1::AttachedValueTransferFailed,
        RuntimeTrapCodeV1::UnsupportedOperation,
        RuntimeTrapCodeV1::InvalidHostInput,
    ];
    for (index, code) in expected.into_iter().enumerate() {
        let raw = (index + 1) as u16;
        assert_eq!(code as u16, raw);
        assert_eq!(RuntimeTrapCodeV1::try_from(raw).unwrap(), code);
    }
    assert_eq!(
        RuntimeTrapCodeV1::try_from(0),
        Err(RuntimeAbiError::UnknownTrapCode(0)),
    );
    assert_eq!(
        RuntimeTrapCodeV1::try_from(0x000c),
        Err(RuntimeAbiError::UnknownTrapCode(0x000c)),
    );
}

#[test]
fn context_depth_is_bounded_including_top_level() {
    let mut parts = context_parts(wasm(0x55), ExecutionDomain::Wasm);
    parts.depth = 1;
    assert!(RuntimeCallContextV1::from_trusted_parts(parts.clone()).is_ok());

    parts.depth = MAX_RUNTIME_CALL_DEPTH;
    assert!(RuntimeCallContextV1::from_trusted_parts(parts.clone()).is_ok());

    parts.depth = 0;
    assert_eq!(
        RuntimeCallContextV1::from_trusted_parts(parts.clone()).unwrap_err(),
        RuntimeAbiError::InvalidCallDepth,
    );

    parts.depth = MAX_RUNTIME_CALL_DEPTH + 1;
    assert_eq!(
        RuntimeCallContextV1::from_trusted_parts(parts).unwrap_err(),
        RuntimeAbiError::InvalidCallDepth,
    );
}

#[test]
fn context_target_kind_must_match_execution_domain() {
    assert!(RuntimeCallContextV1::from_trusted_parts(context_parts(evm(0x01), ExecutionDomain::Evm)).is_ok());
    assert!(RuntimeCallContextV1::from_trusted_parts(context_parts(wasm(0x02), ExecutionDomain::Wasm)).is_ok());

    assert_eq!(
        RuntimeCallContextV1::from_trusted_parts(context_parts(evm(0x03), ExecutionDomain::Wasm)).unwrap_err(),
        RuntimeAbiError::ContextDomainTargetMismatch,
    );
    assert_eq!(
        RuntimeCallContextV1::from_trusted_parts(context_parts(wasm(0x04), ExecutionDomain::Evm)).unwrap_err(),
        RuntimeAbiError::ContextDomainTargetMismatch,
    );
    assert_eq!(
        RuntimeCallContextV1::from_trusted_parts(context_parts(wasm(0x05), ExecutionDomain::Native)).unwrap_err(),
        RuntimeAbiError::ContextDomainTargetMismatch,
    );
}

#[test]
fn trusted_context_preserves_read_only_and_identity_fields() {
    let parts = context_parts(wasm(0x66), ExecutionDomain::Wasm);
    let context = RuntimeCallContextV1::from_trusted_parts(parts.clone()).unwrap();
    assert_eq!(context.chain_id(), parts.chain_id);
    assert_eq!(context.height(), parts.height);
    assert_eq!(context.parent_block_hash(), parts.parent_block_hash);
    assert_eq!(context.txid(), parts.txid);
    assert_eq!(context.principal(), parts.principal);
    assert_eq!(context.caller(), parts.caller);
    assert_eq!(context.target(), parts.target);
    assert_eq!(context.execution_domain(), parts.execution_domain);
    assert_eq!(context.depth(), parts.depth);
    assert_eq!(context.read_only(), parts.read_only);
    assert_eq!(context.transferred_value(), parts.transferred_value);
}

fn host_trait_object_compiles(_: Option<&mut dyn RuntimeHostV1>) {}
fn backend_trait_object_compiles(_: Option<&mut dyn RuntimeBackendV1>) {}

#[test]
fn runtime_traits_are_object_safe() {
    host_trait_object_compiles(None);
    backend_trait_object_compiles(None);
}
