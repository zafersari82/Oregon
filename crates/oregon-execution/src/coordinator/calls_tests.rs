use oregon_contract_state::{
    DomainSnapshot, StateError, StateNode, StateSource, empty_hashes, encode_accounting_u64,
    total_execution_balance_key, MAX_STATE_KEY_BYTES,
};
use oregon_primitives::Hash256;
use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::execution_envelope::ExecutionDomain;
use oregon_primitives::execution_event::MAX_EXECUTION_EVENTS_V1;
use oregon_primitives::state_commitment::CommitmentDomainId;
use oregon_runtime::{
    RuntimeBackendFailureV1, RuntimeBackendV1, RuntimeCallContextV1, RuntimeCallContextV1Parts,
    RuntimeCallResultV1, RuntimeCallSpecV1, RuntimeHostSignalV1, RuntimeHostV1, RuntimeTrapCodeV1,
};

use crate::{
    ExecutionJournalV1, JournalContextV1, JournalLimitsV1, MeterScheduleV1, WeightMeter,
    WeightRatio,
};

use super::accounting::{read_balance, read_total_execution_balance, write_balance};
use super::calls::{
    AttachedValueTransferErrorV1, RuntimeBackendFactoryV1, RuntimeDispatchTableV1,
    derive_child_context, scoped_wasm_storage_key, transfer_attached_value,
};
use super::effects::EffectStackV1;
use super::host::{CoordinatorHostV1, HostChargeScheduleV1, HostChargeScheduleV1Parts};
use super::types::{CoordinatorLimitsV1, CoordinatorTerminalV1};

#[derive(Debug, Default)]
struct EmptySource;

impl StateSource for EmptySource {
    fn get_node(&self, _node_hash: &Hash256) -> Result<Option<StateNode>, StateError> {
        Ok(None)
    }

    fn get_value(&self, _value_hash: &Hash256) -> Result<Option<Vec<u8>>, StateError> {
        Ok(None)
    }
}

fn address(kind: ExecutionAddressKind, byte: u8) -> ExecutionAddress {
    match kind {
        ExecutionAddressKind::Evm => ExecutionAddress::from_evm([byte; 20]),
        _ => ExecutionAddress::new(kind, [byte; 32]).unwrap(),
    }
}

fn journal(source: &EmptySource) -> ExecutionJournalV1<'_, EmptySource> {
    let wasm = CommitmentDomainId::Wasm;
    let accounting = CommitmentDomainId::ExecutionAccounting;
    let snapshots = [
        DomainSnapshot {
            domain: wasm,
            root: empty_hashes(wasm)[0],
        },
        DomainSnapshot {
            domain: accounting,
            root: empty_hashes(accounting)[0],
        },
    ];
    ExecutionJournalV1::new(
        source,
        JournalContextV1 {
            chain_id: 42,
            height: 9001,
            parent_block_hash: Hash256::from_bytes([0x11; 32]),
            txid: Hash256::from_bytes([0x22; 32]),
        },
        &snapshots,
        JournalLimitsV1::default(),
    )
    .unwrap()
}

fn seed_accounting(
    journal: &mut ExecutionJournalV1<'_, EmptySource>,
    caller: ExecutionAddress,
    caller_balance: u64,
    target: ExecutionAddress,
    target_balance: u64,
) {
    write_balance(journal, caller, caller_balance).unwrap();
    write_balance(journal, target, target_balance).unwrap();
    journal
        .put(
            CommitmentDomainId::ExecutionAccounting,
            total_execution_balance_key(),
            &encode_accounting_u64(caller_balance + target_balance),
        )
        .unwrap();
}

fn context(target: ExecutionAddress, depth: u16, read_only: bool) -> RuntimeCallContextV1 {
    let execution_domain = match target.kind() {
        ExecutionAddressKind::Evm => ExecutionDomain::Evm,
        ExecutionAddressKind::Wasm => ExecutionDomain::Wasm,
        _ => panic!("runtime test context requires EVM or WASM target"),
    };
    RuntimeCallContextV1::from_trusted_parts(RuntimeCallContextV1Parts {
        chain_id: 42,
        height: 9001,
        parent_block_hash: Hash256::from_bytes([0x11; 32]),
        txid: Hash256::from_bytes([0x22; 32]),
        principal: address(ExecutionAddressKind::Wasm, 0x31),
        caller: address(ExecutionAddressKind::Wasm, 0x32),
        target,
        execution_domain,
        depth,
        read_only,
        transferred_value: 0,
    })
    .unwrap()
}

fn meter() -> WeightMeter {
    let one = WeightRatio::new(1, 1).unwrap();
    let schedule = MeterScheduleV1::new(1, one, one, one).unwrap();
    WeightMeter::new(schedule, 10_000_000, 0).unwrap()
}

fn effects() -> EffectStackV1 {
    EffectStackV1::new(
        CoordinatorLimitsV1::new(64, MAX_EXECUTION_EVENTS_V1, 2_097_152).unwrap(),
    )
}

fn host_charges() -> HostChargeScheduleV1 {
    HostChargeScheduleV1::new(HostChargeScheduleV1Parts {
        version: 1,
        state_read_base: 1,
        state_read_key_byte: 1,
        state_read_copy_byte: 1,
        state_write_base: 1,
        state_write_key_byte: 1,
        state_write_value_byte: 1,
        state_delete_base: 1,
        state_delete_key_byte: 1,
        event_base: 1,
        event_topic: 1,
        event_data_byte: 1,
        nested_call_base: 1,
        nested_call_input_byte: 1,
        nested_call_return_copy_byte: 1,
        context_query: 1,
    })
    .unwrap()
}

#[test]
fn wasm_storage_scope_binds_the_full_target_identity() {
    let first = address(ExecutionAddressKind::Wasm, 0x41);
    let second = address(ExecutionAddressKind::Wasm, 0x42);
    let local_key = b"same-local-key";

    let first_key = scoped_wasm_storage_key(first, local_key).unwrap();
    let second_key = scoped_wasm_storage_key(second, local_key).unwrap();

    assert_ne!(first_key, second_key);
    let mut expected = b"wasm/v1/storage/".to_vec();
    expected.extend_from_slice(&first.to_bytes());
    expected.extend_from_slice(local_key);
    assert_eq!(first_key, expected);
}

#[test]
fn wasm_scope_rejects_one_over_owner_key_limit_before_copy() {
    let target = address(ExecutionAddressKind::Wasm, 0x41);
    let overhead = b"wasm/v1/storage/".len() + 33;
    let exact = vec![0u8; MAX_STATE_KEY_BYTES - overhead];
    let one_over = vec![0u8; MAX_STATE_KEY_BYTES - overhead + 1];

    assert_eq!(scoped_wasm_storage_key(target, &exact).unwrap().len(), MAX_STATE_KEY_BYTES);
    assert_eq!(
        scoped_wasm_storage_key(target, &one_over),
        Err(RuntimeTrapCodeV1::InvalidHostInput)
    );
}

#[test]
fn two_wasm_targets_using_the_same_local_key_do_not_collide() {
    let source = EmptySource;
    let mut journal = journal(&source);
    journal.begin_frame().unwrap();
    let first = address(ExecutionAddressKind::Wasm, 0x41);
    let second = address(ExecutionAddressKind::Wasm, 0x42);
    let first_context = context(first, 1, false);
    let second_context = context(second, 1, false);
    let dispatch = RuntimeDispatchTableV1::new(&[]).unwrap();
    let mut meter = meter();
    let mut effects = effects();
    effects.begin().unwrap();
    let mut terminal = CoordinatorTerminalV1::Running;

    {
        let mut host = CoordinatorHostV1::new_active(
            &first_context,
            &mut journal,
            &mut meter,
            &mut effects,
            &mut terminal,
            host_charges(),
            &dispatch,
        );
        host.state_put(b"key", b"first").unwrap();
    }
    {
        let mut host = CoordinatorHostV1::new_active(
            &second_context,
            &mut journal,
            &mut meter,
            &mut effects,
            &mut terminal,
            host_charges(),
            &dispatch,
        );
        host.state_put(b"key", b"second").unwrap();
    }

    assert_eq!(
        journal
            .read(
                CommitmentDomainId::Wasm,
                &scoped_wasm_storage_key(first, b"key").unwrap(),
            )
            .unwrap(),
        Some(b"first".to_vec())
    );
    assert_eq!(
        journal
            .read(
                CommitmentDomainId::Wasm,
                &scoped_wasm_storage_key(second, b"key").unwrap(),
            )
            .unwrap(),
        Some(b"second".to_vec())
    );
}

#[test]
fn evm_labelled_backend_is_denied_production_state_access() {
    let source = EmptySource;
    let mut journal = journal(&source);
    journal.begin_frame().unwrap();
    let current = context(address(ExecutionAddressKind::Evm, 0x51), 1, false);
    let dispatch = RuntimeDispatchTableV1::new(&[]).unwrap();
    let mut meter = meter();
    let mut effects = effects();
    effects.begin().unwrap();
    let mut terminal = CoordinatorTerminalV1::Running;
    let mut host = CoordinatorHostV1::new_active(
        &current,
        &mut journal,
        &mut meter,
        &mut effects,
        &mut terminal,
        host_charges(),
        &dispatch,
    );

    assert_eq!(
        host.state_get(b"key"),
        Err(RuntimeHostSignalV1::Trap(RuntimeTrapCodeV1::StateAccessDenied))
    );
    assert_eq!(
        host.state_put(b"key", b"value"),
        Err(RuntimeHostSignalV1::Trap(RuntimeTrapCodeV1::StateAccessDenied))
    );
    assert_eq!(
        host.state_delete(b"key"),
        Err(RuntimeHostSignalV1::Trap(RuntimeTrapCodeV1::StateAccessDenied))
    );
}

#[test]
fn read_only_is_monotonic_across_nested_calls_and_blocks_writes() {
    let parent_target = address(ExecutionAddressKind::Wasm, 0x61);
    let child_target = address(ExecutionAddressKind::Wasm, 0x62);
    let parent = context(parent_target, 1, true);
    let spec = RuntimeCallSpecV1::new(child_target, 0, false, Vec::new()).unwrap();
    let child = derive_child_context(&parent, &spec).unwrap();

    assert!(child.read_only());
    assert_eq!(child.caller(), parent.target());
    assert_eq!(child.target(), child_target);

    let source = EmptySource;
    let mut journal = journal(&source);
    journal.begin_frame().unwrap();
    let dispatch = RuntimeDispatchTableV1::new(&[]).unwrap();
    let mut meter = meter();
    let mut effects = effects();
    effects.begin().unwrap();
    let mut terminal = CoordinatorTerminalV1::Running;
    let mut host = CoordinatorHostV1::new_active(
        &child,
        &mut journal,
        &mut meter,
        &mut effects,
        &mut terminal,
        host_charges(),
        &dispatch,
    );

    assert_eq!(
        host.state_put(b"key", b"value"),
        Err(RuntimeHostSignalV1::Trap(RuntimeTrapCodeV1::ReadOnlyViolation))
    );
    assert_eq!(
        host.state_delete(b"key"),
        Err(RuntimeHostSignalV1::Trap(RuntimeTrapCodeV1::ReadOnlyViolation))
    );
}

#[test]
fn child_depth_accepts_exact_64_and_traps_65_before_frame_growth() {
    let target = address(ExecutionAddressKind::Wasm, 0x72);
    let spec = RuntimeCallSpecV1::new(target, 0, false, Vec::new()).unwrap();
    let at_63 = context(address(ExecutionAddressKind::Wasm, 0x71), 63, false);
    let at_64 = context(address(ExecutionAddressKind::Wasm, 0x71), 64, false);

    assert_eq!(derive_child_context(&at_63, &spec).unwrap().depth(), 64);
    assert_eq!(
        derive_child_context(&at_64, &spec),
        Err(RuntimeTrapCodeV1::CallDepthExceeded)
    );
}

#[test]
fn attached_value_transfer_is_revertible_and_preserves_total_execution_balance() {
    let source = EmptySource;
    let mut journal = journal(&source);
    let caller = address(ExecutionAddressKind::Evm, 0x81);
    let target = address(ExecutionAddressKind::Wasm, 0x82);
    seed_accounting(&mut journal, caller, 100, target, 50);
    journal.begin_frame().unwrap();

    transfer_attached_value(&mut journal, caller, target, 30).unwrap();
    assert_eq!(read_balance(&journal, caller).unwrap(), 70);
    assert_eq!(read_balance(&journal, target).unwrap(), 80);
    assert_eq!(read_total_execution_balance(&journal).unwrap(), 150);

    journal.revert_frame().unwrap();
    assert_eq!(read_balance(&journal, caller).unwrap(), 100);
    assert_eq!(read_balance(&journal, target).unwrap(), 50);
    assert_eq!(read_total_execution_balance(&journal).unwrap(), 150);
}

#[test]
fn insufficient_or_system_value_transfer_fails_with_attached_value_trap_without_mutation() {
    let source = EmptySource;
    let mut journal = journal(&source);
    let caller = address(ExecutionAddressKind::Wasm, 0x91);
    let target = address(ExecutionAddressKind::Evm, 0x92);
    seed_accounting(&mut journal, caller, 10, target, 20);
    journal.begin_frame().unwrap();

    assert_eq!(
        transfer_attached_value(&mut journal, caller, target, 11),
        Err(AttachedValueTransferErrorV1::Trap(
            RuntimeTrapCodeV1::AttachedValueTransferFailed
        ))
    );
    assert_eq!(read_balance(&journal, caller).unwrap(), 10);
    assert_eq!(read_balance(&journal, target).unwrap(), 20);

    let system = address(ExecutionAddressKind::System, 0x93);
    assert_eq!(
        transfer_attached_value(&mut journal, system, target, 1),
        Err(AttachedValueTransferErrorV1::Trap(
            RuntimeTrapCodeV1::AttachedValueTransferFailed
        ))
    );
    let oregon = address(ExecutionAddressKind::Oregon, 0x94);
    assert_eq!(
        transfer_attached_value(&mut journal, caller, oregon, 1),
        Err(AttachedValueTransferErrorV1::Trap(
            RuntimeTrapCodeV1::AttachedValueTransferFailed
        ))
    );
}

struct EmitSuccess;

impl RuntimeBackendV1 for EmitSuccess {
    fn execute(
        &mut self,
        host: &mut dyn RuntimeHostV1,
    ) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1> {
        host.emit_event(&[], b"child-success")
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        Ok(RuntimeCallResultV1::success(b"ok".to_vec()).unwrap())
    }
}

struct EmitRevert;

impl RuntimeBackendV1 for EmitRevert {
    fn execute(
        &mut self,
        host: &mut dyn RuntimeHostV1,
    ) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1> {
        host.emit_event(&[], b"child-revert")
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        Ok(RuntimeCallResultV1::revert(b"no".to_vec()).unwrap())
    }
}

struct EmitTrap;

impl RuntimeBackendV1 for EmitTrap {
    fn execute(
        &mut self,
        host: &mut dyn RuntimeHostV1,
    ) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1> {
        host.emit_event(&[], b"child-trap")
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        Ok(RuntimeCallResultV1::trap(
            RuntimeTrapCodeV1::BackendDeterministic,
        ))
    }
}

fn success_factory() -> Box<dyn RuntimeBackendV1> {
    Box::new(EmitSuccess)
}

fn revert_factory() -> Box<dyn RuntimeBackendV1> {
    Box::new(EmitRevert)
}

fn trap_factory() -> Box<dyn RuntimeBackendV1> {
    Box::new(EmitTrap)
}

fn dispatch(
    target: ExecutionAddress,
    factory: RuntimeBackendFactoryV1,
) -> RuntimeDispatchTableV1 {
    RuntimeDispatchTableV1::new(&[(target, factory)]).unwrap()
}

#[test]
fn nested_success_commits_value_and_event_but_ancestor_revert_discards_both() {
    let source = EmptySource;
    let mut journal = journal(&source);
    let caller = address(ExecutionAddressKind::Wasm, 0xa1);
    let target = address(ExecutionAddressKind::Evm, 0xa2);
    seed_accounting(&mut journal, caller, 100, target, 50);
    journal.begin_frame().unwrap();
    let mut effects = effects();
    effects.begin().unwrap();
    let current = context(caller, 1, false);
    let dispatch = dispatch(target, success_factory as RuntimeBackendFactoryV1);
    let mut meter = meter();
    let mut terminal = CoordinatorTerminalV1::Running;

    let result = {
        let mut host = CoordinatorHostV1::new_active(
            &current,
            &mut journal,
            &mut meter,
            &mut effects,
            &mut terminal,
            host_charges(),
            &dispatch,
        );
        host.call(RuntimeCallSpecV1::new(target, 30, false, Vec::new()).unwrap())
            .unwrap()
    };
    assert!(matches!(result, RuntimeCallResultV1::Success(_)));
    assert_eq!(read_balance(&journal, caller).unwrap(), 70);
    assert_eq!(read_balance(&journal, target).unwrap(), 80);
    assert_eq!(read_total_execution_balance(&journal).unwrap(), 150);

    journal.revert_frame().unwrap();
    effects.revert().unwrap();
    assert_eq!(read_balance(&journal, caller).unwrap(), 100);
    assert_eq!(read_balance(&journal, target).unwrap(), 50);
    assert_eq!(read_total_execution_balance(&journal).unwrap(), 150);
    assert!(effects.root_events().is_empty());
}

#[test]
fn nested_revert_and_trap_discard_child_value_and_events() {
    for (factory, expect_trap) in [
        (revert_factory as RuntimeBackendFactoryV1, false),
        (trap_factory as RuntimeBackendFactoryV1, true),
    ] {
        let source = EmptySource;
        let mut journal = journal(&source);
        let caller = address(ExecutionAddressKind::Wasm, 0xb1);
        let target = address(ExecutionAddressKind::Wasm, 0xb2);
        seed_accounting(&mut journal, caller, 100, target, 50);
        journal.begin_frame().unwrap();
        let mut effects = effects();
        effects.begin().unwrap();
        let current = context(caller, 1, false);
        let dispatch = dispatch(target, factory);
        let mut meter = meter();
        let mut terminal = CoordinatorTerminalV1::Running;

        let result = {
            let mut host = CoordinatorHostV1::new_active(
                &current,
                &mut journal,
                &mut meter,
                &mut effects,
                &mut terminal,
                host_charges(),
                &dispatch,
            );
            host.call(RuntimeCallSpecV1::new(target, 30, false, Vec::new()).unwrap())
                .unwrap()
        };

        if expect_trap {
            assert_eq!(
                result,
                RuntimeCallResultV1::trap(RuntimeTrapCodeV1::BackendDeterministic)
            );
        } else {
            assert!(matches!(result, RuntimeCallResultV1::Revert(_)));
        }
        assert_eq!(read_balance(&journal, caller).unwrap(), 100);
        assert_eq!(read_balance(&journal, target).unwrap(), 50);
        assert_eq!(read_total_execution_balance(&journal).unwrap(), 150);
        assert!(effects.root_events().is_empty());
    }
}

#[test]
fn insufficient_nested_value_returns_0009_without_call_effects() {
    let source = EmptySource;
    let mut journal = journal(&source);
    let caller = address(ExecutionAddressKind::Wasm, 0xc1);
    let target = address(ExecutionAddressKind::Wasm, 0xc2);
    seed_accounting(&mut journal, caller, 5, target, 50);
    journal.begin_frame().unwrap();
    let mut effects = effects();
    effects.begin().unwrap();
    let current = context(caller, 1, false);
    let dispatch = dispatch(target, success_factory as RuntimeBackendFactoryV1);
    let mut meter = meter();
    let mut terminal = CoordinatorTerminalV1::Running;

    let result = {
        let mut host = CoordinatorHostV1::new_active(
            &current,
            &mut journal,
            &mut meter,
            &mut effects,
            &mut terminal,
            host_charges(),
            &dispatch,
        );
        host.call(RuntimeCallSpecV1::new(target, 6, false, Vec::new()).unwrap())
            .unwrap()
    };

    assert_eq!(
        result,
        RuntimeCallResultV1::trap(RuntimeTrapCodeV1::AttachedValueTransferFailed)
    );
    assert_eq!(read_balance(&journal, caller).unwrap(), 5);
    assert_eq!(read_balance(&journal, target).unwrap(), 50);
    assert!(effects.root_events().is_empty());
}
