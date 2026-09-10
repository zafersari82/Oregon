use std::cell::RefCell;

use oregon_primitives::Hash256;
use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::execution_envelope::ExecutionDomain;
use oregon_primitives::execution_event::MAX_EXECUTION_EVENTS_V1;
use oregon_primitives::state_commitment::CommitmentDomainId;
use oregon_runtime::{
    RuntimeBackendFailureV1, RuntimeBackendV1, RuntimeCallContextV1, RuntimeCallContextV1Parts,
    RuntimeCallResultV1, RuntimeCallSpecV1, RuntimeHostSignalV1, RuntimeHostV1,
    RuntimeTrapCodeV1,
};

use crate::{JournalError, MeterScheduleV1, WeightMeter, WeightRatio};

use super::accounting::CoordinatorJournalV1;
use super::calls::RuntimeDispatchTableV1;
use super::effects::EffectStackV1;
use super::host::{CoordinatorHostV1, HostChargeScheduleV1, HostChargeScheduleV1Parts};
use super::types::{CoordinatorLimitsV1, CoordinatorTerminalV1};

fn address(kind: ExecutionAddressKind, byte: u8) -> ExecutionAddress {
    match kind {
        ExecutionAddressKind::Evm => ExecutionAddress::from_evm([byte; 20]),
        _ => ExecutionAddress::new(kind, [byte; 32]).unwrap(),
    }
}

#[derive(Clone)]
enum ScriptOp {
    Read(Vec<u8>),
    Put(Vec<u8>, Vec<u8>),
    Emit(Vec<Hash256>, Vec<u8>),
    ChargeVm(u64),
    ChargeCommon(u64),
    Call(RuntimeCallSpecV1),
    Return(Vec<u8>),
    Revert(Vec<u8>),
    Trap(RuntimeTrapCodeV1),
    IgnoreAbortThenReturnSuccess,
}

struct ScriptedBackend {
    script: Vec<ScriptOp>,
}

impl ScriptedBackend {
    fn new(script: Vec<ScriptOp>) -> Self {
        Self { script }
    }

    fn operation_count(&self) -> usize {
        self.script.len()
    }
}

fn signal_as_backend_result(
    signal: RuntimeHostSignalV1,
) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1> {
    match signal {
        RuntimeHostSignalV1::Trap(code) => Ok(RuntimeCallResultV1::trap(code)),
        RuntimeHostSignalV1::Abort => Err(RuntimeBackendFailureV1::Fatal),
    }
}

impl RuntimeBackendV1 for ScriptedBackend {
    fn execute(
        &mut self,
        host: &mut dyn RuntimeHostV1,
    ) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1> {
        for op in self.script.clone() {
            match op {
                ScriptOp::Read(key) => {
                    if let Err(signal) = host.state_get(&key) {
                        return signal_as_backend_result(signal);
                    }
                }
                ScriptOp::Put(key, value) => {
                    if let Err(signal) = host.state_put(&key, &value) {
                        return signal_as_backend_result(signal);
                    }
                }
                ScriptOp::Emit(topics, data) => {
                    if let Err(signal) = host.emit_event(&topics, &data) {
                        return signal_as_backend_result(signal);
                    }
                }
                ScriptOp::ChargeVm(units) => {
                    if let Err(signal) = host.charge_vm_units(units) {
                        return signal_as_backend_result(signal);
                    }
                }
                ScriptOp::ChargeCommon(units) => {
                    if let Err(signal) = host.charge_common(units) {
                        return signal_as_backend_result(signal);
                    }
                }
                ScriptOp::Call(spec) => match host.call(spec) {
                    Ok(RuntimeCallResultV1::Success(_)) => {}
                    Ok(result) => return Ok(result),
                    Err(signal) => return signal_as_backend_result(signal),
                },
                ScriptOp::Return(data) => {
                    return RuntimeCallResultV1::success(data)
                        .map_err(|_| RuntimeBackendFailureV1::Fatal);
                }
                ScriptOp::Revert(data) => {
                    return RuntimeCallResultV1::revert(data)
                        .map_err(|_| RuntimeBackendFailureV1::Fatal);
                }
                ScriptOp::Trap(code) => return Ok(RuntimeCallResultV1::trap(code)),
                ScriptOp::IgnoreAbortThenReturnSuccess => {
                    let _ = host.charge_common(u64::MAX);
                    return Ok(RuntimeCallResultV1::Success(Vec::new()));
                }
            }
        }
        Ok(RuntimeCallResultV1::Success(Vec::new()))
    }
}

#[derive(Debug)]
struct TestJournal {
    depth: usize,
    fail_reads: bool,
}

impl TestJournal {
    fn new() -> Self {
        Self {
            depth: 1,
            fail_reads: false,
        }
    }

    fn with_failing_reads() -> Self {
        Self {
            depth: 1,
            fail_reads: true,
        }
    }

    fn end_frame(&mut self) -> Result<(), JournalError> {
        if self.depth <= 1 {
            return Err(JournalError::RootFrameLifecycle);
        }
        self.depth -= 1;
        Ok(())
    }
}

impl CoordinatorJournalV1 for TestJournal {
    fn read(
        &self,
        _domain: CommitmentDomainId,
        _key: &[u8],
    ) -> Result<Option<Vec<u8>>, JournalError> {
        if self.fail_reads {
            Err(JournalError::AccountingInvariant)
        } else {
            Ok(None)
        }
    }

    fn put(
        &mut self,
        _domain: CommitmentDomainId,
        _key: &[u8],
        _value: &[u8],
    ) -> Result<(), JournalError> {
        Ok(())
    }

    fn delete(
        &mut self,
        _domain: CommitmentDomainId,
        _key: &[u8],
    ) -> Result<(), JournalError> {
        Ok(())
    }

    fn begin_frame(&mut self) -> Result<(), JournalError> {
        self.depth += 1;
        Ok(())
    }

    fn commit_frame(&mut self) -> Result<(), JournalError> {
        self.end_frame()
    }

    fn revert_frame(&mut self) -> Result<(), JournalError> {
        self.end_frame()
    }
}

fn meter(max_weight: u64) -> WeightMeter {
    let schedule = MeterScheduleV1::new(
        1,
        WeightRatio::new(1, 1).unwrap(),
        WeightRatio::new(1, 1).unwrap(),
        WeightRatio::new(1, 1).unwrap(),
    )
    .unwrap();
    WeightMeter::new(schedule, max_weight, 0).unwrap()
}

fn limits() -> CoordinatorLimitsV1 {
    CoordinatorLimitsV1::new(64, MAX_EXECUTION_EVENTS_V1, 2_097_152).unwrap()
}

fn effects() -> EffectStackV1 {
    EffectStackV1::new(limits())
}

fn charges() -> HostChargeScheduleV1 {
    HostChargeScheduleV1::new(HostChargeScheduleV1Parts {
        version: 1,
        state_read_base: 1,
        state_read_key_byte: 0,
        state_read_copy_byte: 0,
        state_write_base: 1,
        state_write_key_byte: 0,
        state_write_value_byte: 0,
        state_delete_base: 1,
        state_delete_key_byte: 0,
        event_base: 1,
        event_topic: 0,
        event_data_byte: 0,
        nested_call_base: 1,
        nested_call_input_byte: 0,
        nested_call_return_copy_byte: 0,
        context_query: 1,
    })
    .unwrap()
}

fn principal() -> ExecutionAddress {
    address(ExecutionAddressKind::Wasm, 0x01)
}

fn runtime_context(
    target: ExecutionAddress,
    execution_domain: ExecutionDomain,
    caller: ExecutionAddress,
    depth: u16,
    read_only: bool,
) -> RuntimeCallContextV1 {
    RuntimeCallContextV1::from_trusted_parts(RuntimeCallContextV1Parts {
        chain_id: 42,
        height: 9001,
        parent_block_hash: Hash256::from_bytes([0x11; 32]),
        txid: Hash256::from_bytes([0x22; 32]),
        principal: principal(),
        caller,
        target,
        execution_domain,
        depth,
        read_only,
        transferred_value: 0,
    })
    .unwrap()
}

#[derive(Clone, Copy)]
struct ObservedContext {
    domain: u8,
    depth: u16,
    read_only: bool,
    caller: ExecutionAddress,
    target: ExecutionAddress,
}

std::thread_local! {
    static CONTEXT_TRACE: RefCell<Vec<ObservedContext>> = RefCell::new(Vec::new());
}

fn clear_context_trace() {
    CONTEXT_TRACE.with(|trace| trace.borrow_mut().clear());
}

fn record_context(context: &RuntimeCallContextV1) {
    CONTEXT_TRACE.with(|trace| {
        trace.borrow_mut().push(ObservedContext {
            domain: context.execution_domain() as u8,
            depth: context.depth(),
            read_only: context.read_only(),
            caller: context.caller(),
            target: context.target(),
        });
    });
}

fn take_context_trace() -> Vec<ObservedContext> {
    CONTEXT_TRACE.with(|trace| std::mem::take(&mut *trace.borrow_mut()))
}

fn top_trace_target() -> ExecutionAddress {
    address(ExecutionAddressKind::Wasm, 0x10)
}

fn evm_trace_target() -> ExecutionAddress {
    address(ExecutionAddressKind::Evm, 0x20)
}

fn wasm_leaf_target() -> ExecutionAddress {
    address(ExecutionAddressKind::Wasm, 0x30)
}

struct WasmLeafBackend;

impl RuntimeBackendV1 for WasmLeafBackend {
    fn execute(
        &mut self,
        host: &mut dyn RuntimeHostV1,
    ) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1> {
        record_context(host.context());
        host.charge_vm_units(1)
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        Ok(RuntimeCallResultV1::Success(Vec::new()))
    }
}

fn wasm_leaf_backend() -> Box<dyn RuntimeBackendV1> {
    Box::new(WasmLeafBackend)
}

struct EvmTraceBackend;

impl RuntimeBackendV1 for EvmTraceBackend {
    fn execute(
        &mut self,
        host: &mut dyn RuntimeHostV1,
    ) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1> {
        record_context(host.context());
        host.charge_vm_units(1)
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        let call = RuntimeCallSpecV1::new(wasm_leaf_target(), 0, false, Vec::new())
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        host.call(call).map_err(|signal| match signal {
            RuntimeHostSignalV1::Trap(_) | RuntimeHostSignalV1::Abort => {
                RuntimeBackendFailureV1::Fatal
            }
        })
    }
}

fn evm_trace_backend() -> Box<dyn RuntimeBackendV1> {
    Box::new(EvmTraceBackend)
}

struct WasmTraceRootBackend;

impl RuntimeBackendV1 for WasmTraceRootBackend {
    fn execute(
        &mut self,
        host: &mut dyn RuntimeHostV1,
    ) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1> {
        record_context(host.context());
        host.charge_vm_units(1)
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        let call = RuntimeCallSpecV1::new(evm_trace_target(), 0, false, Vec::new())
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        host.call(call).map_err(|signal| match signal {
            RuntimeHostSignalV1::Trap(_) | RuntimeHostSignalV1::Abort => {
                RuntimeBackendFailureV1::Fatal
            }
        })
    }
}

struct WasmEvmWasmObservation {
    domains: [u8; 3],
    depths: [u16; 3],
    read_only: [bool; 3],
    callers: [ExecutionAddress; 3],
    targets: [ExecutionAddress; 3],
    consumed_weight: u64,
}

fn run_wasm_evm_wasm_trace() -> WasmEvmWasmObservation {
    clear_context_trace();
    let context = runtime_context(
        top_trace_target(),
        ExecutionDomain::Wasm,
        principal(),
        1,
        true,
    );
    let dispatch = RuntimeDispatchTableV1::new(&[
        (evm_trace_target(), evm_trace_backend),
        (wasm_leaf_target(), wasm_leaf_backend),
    ])
    .unwrap();
    let mut journal = TestJournal::new();
    journal.begin_frame().unwrap();
    let mut meter = meter(100);
    let mut effects = effects();
    effects.begin().unwrap();
    let mut terminal = CoordinatorTerminalV1::Running;
    let mut backend = WasmTraceRootBackend;

    let result = {
        let mut host = CoordinatorHostV1::new_active(
            &context,
            &mut journal,
            &mut meter,
            &mut effects,
            &mut terminal,
            charges(),
            &dispatch,
        );
        backend.execute(&mut host).unwrap()
    };
    assert!(matches!(result, RuntimeCallResultV1::Success(_)));
    assert_eq!(terminal, CoordinatorTerminalV1::Running);
    journal.commit_frame().unwrap();
    effects.commit().unwrap();

    let trace = take_context_trace();
    assert_eq!(trace.len(), 3);
    WasmEvmWasmObservation {
        domains: [trace[0].domain, trace[1].domain, trace[2].domain],
        depths: [trace[0].depth, trace[1].depth, trace[2].depth],
        read_only: [trace[0].read_only, trace[1].read_only, trace[2].read_only],
        callers: [trace[0].caller, trace[1].caller, trace[2].caller],
        targets: [trace[0].target, trace[1].target, trace[2].target],
        consumed_weight: meter.consumed(),
    }
}

fn run_evm_state_access_trace() -> RuntimeTrapCodeV1 {
    let context = runtime_context(
        address(ExecutionAddressKind::Evm, 0x50),
        ExecutionDomain::Evm,
        principal(),
        1,
        false,
    );
    let dispatch = RuntimeDispatchTableV1::new(&[]).unwrap();
    let mut journal = TestJournal::new();
    let mut meter = meter(100);
    let mut effects = effects();
    let mut terminal = CoordinatorTerminalV1::Running;
    let mut backend = ScriptedBackend::new(vec![ScriptOp::Read(b"denied".to_vec())]);
    let result = {
        let mut host = CoordinatorHostV1::new_active(
            &context,
            &mut journal,
            &mut meter,
            &mut effects,
            &mut terminal,
            charges(),
            &dispatch,
        );
        backend.execute(&mut host).unwrap()
    };
    result.trap_code().expect("EVM state access must trap")
}

fn revert_child_target() -> ExecutionAddress {
    address(ExecutionAddressKind::Wasm, 0x61)
}

fn trap_child_target() -> ExecutionAddress {
    address(ExecutionAddressKind::Wasm, 0x62)
}

struct RevertChildBackend;

impl RuntimeBackendV1 for RevertChildBackend {
    fn execute(
        &mut self,
        host: &mut dyn RuntimeHostV1,
    ) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1> {
        host.emit_event(&[], b"reverted-child")
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        Ok(RuntimeCallResultV1::Revert(b"child-revert".to_vec()))
    }
}

fn revert_child_backend() -> Box<dyn RuntimeBackendV1> {
    Box::new(RevertChildBackend)
}

struct TrapChildBackend;

impl RuntimeBackendV1 for TrapChildBackend {
    fn execute(
        &mut self,
        host: &mut dyn RuntimeHostV1,
    ) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1> {
        host.emit_event(&[], b"trapped-child")
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        Ok(RuntimeCallResultV1::trap(
            RuntimeTrapCodeV1::BackendDeterministic,
        ))
    }
}

fn trap_child_backend() -> Box<dyn RuntimeBackendV1> {
    Box::new(TrapChildBackend)
}

struct RevertTrapParentBackend {
    continued_after_revert: bool,
    continued_after_trap: bool,
}

impl RuntimeBackendV1 for RevertTrapParentBackend {
    fn execute(
        &mut self,
        host: &mut dyn RuntimeHostV1,
    ) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1> {
        let reverted = host
            .call(
                RuntimeCallSpecV1::new(revert_child_target(), 0, false, Vec::new())
                    .map_err(|_| RuntimeBackendFailureV1::Fatal)?,
            )
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        self.continued_after_revert = matches!(reverted, RuntimeCallResultV1::Revert(_));

        let trapped = host
            .call(
                RuntimeCallSpecV1::new(trap_child_target(), 0, false, Vec::new())
                    .map_err(|_| RuntimeBackendFailureV1::Fatal)?,
            )
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        self.continued_after_trap = matches!(trapped, RuntimeCallResultV1::Trap(_));

        host.emit_event(&[], b"parent-event")
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        Ok(RuntimeCallResultV1::Revert(b"ancestor-revert".to_vec()))
    }
}

struct RevertTrapObservation {
    parent_continued_after_revert: bool,
    parent_continued_after_trap: bool,
    committed_event_count: usize,
    live_effect_bytes: usize,
}

fn run_revert_trap_trace() -> RevertTrapObservation {
    let top_target = address(ExecutionAddressKind::Wasm, 0x60);
    let context = runtime_context(top_target, ExecutionDomain::Wasm, principal(), 1, false);
    let dispatch = RuntimeDispatchTableV1::new(&[
        (revert_child_target(), revert_child_backend),
        (trap_child_target(), trap_child_backend),
    ])
    .unwrap();
    let mut journal = TestJournal::new();
    journal.begin_frame().unwrap();
    let mut meter = meter(100);
    let mut effects = effects();
    effects.begin().unwrap();
    let mut terminal = CoordinatorTerminalV1::Running;
    let mut backend = RevertTrapParentBackend {
        continued_after_revert: false,
        continued_after_trap: false,
    };

    let result = {
        let mut host = CoordinatorHostV1::new_active(
            &context,
            &mut journal,
            &mut meter,
            &mut effects,
            &mut terminal,
            charges(),
            &dispatch,
        );
        backend.execute(&mut host).unwrap()
    };
    assert!(matches!(result, RuntimeCallResultV1::Revert(_)));
    assert_eq!(terminal, CoordinatorTerminalV1::Running);
    journal.revert_frame().unwrap();
    effects.revert().unwrap();

    RevertTrapObservation {
        parent_continued_after_revert: backend.continued_after_revert,
        parent_continued_after_trap: backend.continued_after_trap,
        committed_event_count: effects.root_events().len(),
        live_effect_bytes: effects.live_retained_bytes(),
    }
}

struct IgnoreAbortObservation {
    backend_returned_success: bool,
    resource_exhausted: bool,
    consumed_weight: u64,
    max_weight: u64,
}

fn run_ignore_abort_trace() -> IgnoreAbortObservation {
    let context = runtime_context(
        address(ExecutionAddressKind::Wasm, 0x70),
        ExecutionDomain::Wasm,
        principal(),
        1,
        false,
    );
    let mut meter = meter(5);
    let mut effects = effects();
    let mut terminal = CoordinatorTerminalV1::Running;
    let mut backend = ScriptedBackend::new(vec![ScriptOp::IgnoreAbortThenReturnSuccess]);
    let result = {
        let mut host = CoordinatorHostV1::new(
            &context,
            &mut meter,
            &mut effects,
            &mut terminal,
            charges(),
        );
        backend.execute(&mut host).unwrap()
    };

    IgnoreAbortObservation {
        backend_returned_success: matches!(result, RuntimeCallResultV1::Success(_)),
        resource_exhausted: terminal == CoordinatorTerminalV1::ResourceExhausted
            && meter.is_exhausted(),
        consumed_weight: meter.consumed(),
        max_weight: meter.max_weight(),
    }
}

fn fatal_child_target() -> ExecutionAddress {
    address(ExecutionAddressKind::Wasm, 0x81)
}

struct FatalReadThenSuccessBackend;

impl RuntimeBackendV1 for FatalReadThenSuccessBackend {
    fn execute(
        &mut self,
        host: &mut dyn RuntimeHostV1,
    ) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1> {
        let _ = host.state_get(b"fatal-read");
        Ok(RuntimeCallResultV1::Success(Vec::new()))
    }
}

fn fatal_child_backend() -> Box<dyn RuntimeBackendV1> {
    Box::new(FatalReadThenSuccessBackend)
}

struct FatalParentBackend {
    attempted_to_continue: bool,
}

impl RuntimeBackendV1 for FatalParentBackend {
    fn execute(
        &mut self,
        host: &mut dyn RuntimeHostV1,
    ) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1> {
        let call = RuntimeCallSpecV1::new(fatal_child_target(), 0, false, Vec::new())
            .map_err(|_| RuntimeBackendFailureV1::Fatal)?;
        let _ = host.call(call);
        self.attempted_to_continue = true;
        Ok(RuntimeCallResultV1::Success(Vec::new()))
    }
}

struct FatalJournalObservation {
    parent_attempted_to_continue: bool,
    fatal: bool,
    success_escaped: bool,
}

fn run_fatal_journal_trace() -> FatalJournalObservation {
    let top_target = address(ExecutionAddressKind::Wasm, 0x80);
    let context = runtime_context(top_target, ExecutionDomain::Wasm, principal(), 1, false);
    let dispatch = RuntimeDispatchTableV1::new(&[(fatal_child_target(), fatal_child_backend)]).unwrap();
    let mut journal = TestJournal::with_failing_reads();
    journal.begin_frame().unwrap();
    let mut meter = meter(100);
    let mut effects = effects();
    effects.begin().unwrap();
    let mut terminal = CoordinatorTerminalV1::Running;
    let mut backend = FatalParentBackend {
        attempted_to_continue: false,
    };

    let result = {
        let mut host = CoordinatorHostV1::new_active(
            &context,
            &mut journal,
            &mut meter,
            &mut effects,
            &mut terminal,
            charges(),
            &dispatch,
        );
        backend.execute(&mut host).unwrap()
    };
    let success_escaped =
        matches!(result, RuntimeCallResultV1::Success(_)) && terminal == CoordinatorTerminalV1::Running;
    journal.revert_frame().unwrap();
    effects.revert().unwrap();

    FatalJournalObservation {
        parent_attempted_to_continue: backend.attempted_to_continue,
        fatal: terminal == CoordinatorTerminalV1::Fatal,
        success_escaped,
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
        ScriptOp::Trap(RuntimeTrapCodeV1::BackendDeterministic),
        ScriptOp::IgnoreAbortThenReturnSuccess,
    ];
    let backend = ScriptedBackend::new(script);
    assert_eq!(backend.operation_count(), 10);
}

#[test]
fn wasm_evm_wasm_trace_preserves_shared_meter_read_only_and_caller_context() {
    let observation = run_wasm_evm_wasm_trace();
    assert_eq!(observation.domains, [0x12, 0x11, 0x12]);
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
