use oregon_primitives::Hash256;
use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_runtime::{RuntimeCallSpecV1, RuntimeTrapCodeV1};

fn address(kind: ExecutionAddressKind, byte: u8) -> ExecutionAddress {
    match kind {
        ExecutionAddressKind::Evm => ExecutionAddress::from_evm([byte; 20]),
        _ => ExecutionAddress::new(kind, [byte; 32]).unwrap(),
    }
}

#[test]
fn scripted_backend_vocabulary_covers_required_adversarial_operations() {
    let call = RuntimeCallSpecV1::new(address(ExecutionAddressKind::Wasm, 0x44), 0, false, vec![1])
        .unwrap();
    let script = vec![
        ScriptOp::Read(b"k".to_vec()),
        ScriptOp::Put(b"k".to_vec(), b"v".to_vec()),
        ScriptOp::Emit(vec![Hash256::from_bytes([0x11; 32])], b"e".to_vec()),
        ScriptOp::ChargeVm(1),
        ScriptOp::ChargeCommon(2),
        ScriptOp::Call(call),
        ScriptOp::Return(b"ok".to_vec()),
        ScriptOp::Revert(b"no".to_vec()),
        ScriptOp::Trap(RuntimeTrapCodeV1::BackendTrap),
        ScriptOp::IgnoreAbortThenReturnSuccess,
    ];
    let backend = ScriptedBackend::new(script);
    assert_eq!(backend.operation_count(), 10);
}

#[test]
fn wasm_evm_wasm_trace_preserves_shared_meter_read_only_and_caller_context() {
    let observation = run_wasm_evm_wasm_trace();
    assert_eq!(observation.domains, [0x02, 0x01, 0x02]);
    assert_eq!(observation.depths, [1, 2, 3]);
    assert_eq!(observation.read_only, [true, true, true]);
    assert_eq!(observation.callers[1], observation.targets[0]);
    assert_eq!(observation.callers[2], observation.targets[1]);
    assert!(observation.consumed_weight > 0);
}

#[test]
fn evm_labelled_backend_state_access_is_deterministically_denied() {
    assert_eq!(
        run_evm_state_access_trace(),
        RuntimeTrapCodeV1::StateAccessDenied
    );
}

#[test]
fn child_revert_and_trap_are_catchable_but_ancestor_revert_discards_effects() {
    let observation = run_revert_trap_trace();
    assert!(observation.parent_continued_after_revert);
    assert!(observation.parent_continued_after_trap);
    assert_eq!(observation.committed_event_count, 0);
    assert_eq!(observation.live_effect_bytes, 0);
}

#[test]
fn backend_cannot_mask_resource_exhaustion_by_returning_success() {
    let observation = run_ignore_abort_trace();
    assert!(observation.backend_returned_success);
    assert!(observation.resource_exhausted);
    assert_eq!(observation.consumed_weight, observation.max_weight);
}

#[test]
fn fatal_journal_error_cannot_be_converted_to_parent_success() {
    let observation = run_fatal_journal_trace();
    assert!(observation.parent_attempted_to_continue);
    assert!(observation.fatal);
    assert!(!observation.success_escaped);
}
