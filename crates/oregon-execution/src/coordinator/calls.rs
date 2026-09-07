use std::collections::BTreeMap;

use oregon_contract_state::MAX_STATE_KEY_BYTES;
use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::execution_envelope::ExecutionDomain;
use oregon_runtime::{
    MAX_RUNTIME_CALL_DEPTH, RuntimeBackendV1, RuntimeCallContextV1, RuntimeCallContextV1Parts,
    RuntimeCallResultV1, RuntimeCallSpecV1, RuntimeHostSignalV1, RuntimeTrapCodeV1,
};

use crate::WeightMeter;

use super::accounting::{CoordinatorJournalV1, read_balance, write_balance};
use super::effects::EffectStackV1;
use super::host::{CoordinatorHostV1, HostChargeScheduleV1};
use super::types::{CoordinatorError, CoordinatorTerminalV1};

const WASM_STORAGE_PREFIX_V1: &[u8] = b"wasm/v1/storage/";
const EXECUTION_ADDRESS_BYTES: usize = 33;

pub(super) type RuntimeBackendFactoryV1 = fn() -> Box<dyn RuntimeBackendV1>;

#[derive(Default)]
pub(super) struct RuntimeDispatchTableV1 {
    factories: BTreeMap<[u8; EXECUTION_ADDRESS_BYTES], RuntimeBackendFactoryV1>,
}

impl RuntimeDispatchTableV1 {
    pub(super) fn new(
        entries: &[(ExecutionAddress, RuntimeBackendFactoryV1)],
    ) -> Result<Self, CoordinatorError> {
        let mut factories = BTreeMap::new();
        for &(target, factory) in entries {
            if !matches!(
                target.kind(),
                ExecutionAddressKind::Evm | ExecutionAddressKind::Wasm
            ) {
                return Err(CoordinatorError::InvalidRuntimeTarget);
            }
            if factories.insert(target.to_bytes(), factory).is_some() {
                return Err(CoordinatorError::DuplicateRuntimeTarget);
            }
        }
        Ok(Self { factories })
    }

    fn factory_for(&self, target: ExecutionAddress) -> Option<RuntimeBackendFactoryV1> {
        self.factories.get(&target.to_bytes()).copied()
    }
}

pub(super) fn scoped_wasm_storage_key(
    target: ExecutionAddress,
    local_key: &[u8],
) -> Result<Vec<u8>, RuntimeTrapCodeV1> {
    if target.kind() != ExecutionAddressKind::Wasm {
        return Err(RuntimeTrapCodeV1::StateAccessDenied);
    }

    let overhead = WASM_STORAGE_PREFIX_V1
        .len()
        .checked_add(EXECUTION_ADDRESS_BYTES)
        .ok_or(RuntimeTrapCodeV1::InvalidHostInput)?;
    let total_len = overhead
        .checked_add(local_key.len())
        .ok_or(RuntimeTrapCodeV1::InvalidHostInput)?;
    if total_len > MAX_STATE_KEY_BYTES {
        return Err(RuntimeTrapCodeV1::InvalidHostInput);
    }

    let mut key = Vec::with_capacity(total_len);
    key.extend_from_slice(WASM_STORAGE_PREFIX_V1);
    key.extend_from_slice(&target.to_bytes());
    key.extend_from_slice(local_key);
    Ok(key)
}

pub(super) fn derive_child_context(
    parent: &RuntimeCallContextV1,
    spec: &RuntimeCallSpecV1,
) -> Result<RuntimeCallContextV1, RuntimeTrapCodeV1> {
    let depth = parent
        .depth()
        .checked_add(1)
        .ok_or(RuntimeTrapCodeV1::CallDepthExceeded)?;
    if depth > MAX_RUNTIME_CALL_DEPTH {
        return Err(RuntimeTrapCodeV1::CallDepthExceeded);
    }

    let execution_domain = match spec.target().kind() {
        ExecutionAddressKind::Evm => ExecutionDomain::Evm,
        ExecutionAddressKind::Wasm => ExecutionDomain::Wasm,
        ExecutionAddressKind::Oregon | ExecutionAddressKind::System => {
            return Err(RuntimeTrapCodeV1::InvalidCallTarget);
        }
    };

    RuntimeCallContextV1::from_trusted_parts(RuntimeCallContextV1Parts {
        chain_id: parent.chain_id(),
        height: parent.height(),
        parent_block_hash: parent.parent_block_hash(),
        txid: parent.txid(),
        principal: parent.principal(),
        caller: parent.target(),
        target: spec.target(),
        execution_domain,
        depth,
        read_only: parent.read_only() || spec.read_only(),
        transferred_value: spec.value(),
    })
    .map_err(|_| RuntimeTrapCodeV1::InvalidCallTarget)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AttachedValueTransferErrorV1 {
    Trap(RuntimeTrapCodeV1),
    Fatal,
}

pub(super) fn transfer_attached_value(
    journal: &mut dyn CoordinatorJournalV1,
    caller: ExecutionAddress,
    target: ExecutionAddress,
    value: u64,
) -> Result<(), AttachedValueTransferErrorV1> {
    if value == 0 {
        return Ok(());
    }

    let spendable_kind = |kind| matches!(kind, ExecutionAddressKind::Evm | ExecutionAddressKind::Wasm);
    if !spendable_kind(caller.kind()) || !spendable_kind(target.kind()) {
        return Err(AttachedValueTransferErrorV1::Trap(
            RuntimeTrapCodeV1::AttachedValueTransferFailed,
        ));
    }

    let caller_balance = read_balance(journal, caller).map_err(|_| AttachedValueTransferErrorV1::Fatal)?;
    if caller_balance < value {
        return Err(AttachedValueTransferErrorV1::Trap(
            RuntimeTrapCodeV1::AttachedValueTransferFailed,
        ));
    }

    if caller == target {
        return Ok(());
    }

    let target_balance = read_balance(journal, target).map_err(|_| AttachedValueTransferErrorV1::Fatal)?;
    let next_caller = caller_balance
        .checked_sub(value)
        .ok_or(AttachedValueTransferErrorV1::Fatal)?;
    let next_target = target_balance
        .checked_add(value)
        .ok_or(AttachedValueTransferErrorV1::Fatal)?;

    write_balance(journal, caller, next_caller).map_err(|_| AttachedValueTransferErrorV1::Fatal)?;
    write_balance(journal, target, next_target).map_err(|_| AttachedValueTransferErrorV1::Fatal)?;
    Ok(())
}

fn rollback_child_frames(
    journal: &mut dyn CoordinatorJournalV1,
    effects: &mut EffectStackV1,
    terminal: &mut CoordinatorTerminalV1,
) -> Result<(), RuntimeHostSignalV1> {
    let journal_result = journal.revert_frame();
    let effects_result = effects.revert();
    if journal_result.is_err() || effects_result.is_err() {
        *terminal = CoordinatorTerminalV1::Fatal;
        return Err(RuntimeHostSignalV1::Abort);
    }
    Ok(())
}

fn begin_child_frames(
    journal: &mut dyn CoordinatorJournalV1,
    effects: &mut EffectStackV1,
    terminal: &mut CoordinatorTerminalV1,
) -> Result<(), RuntimeHostSignalV1> {
    if journal.begin_frame().is_err() {
        *terminal = CoordinatorTerminalV1::ResourceExhausted;
        return Err(RuntimeHostSignalV1::Abort);
    }
    if effects.begin().is_err() {
        if journal.revert_frame().is_err() {
            *terminal = CoordinatorTerminalV1::Fatal;
        } else {
            *terminal = CoordinatorTerminalV1::ResourceExhausted;
        }
        return Err(RuntimeHostSignalV1::Abort);
    }
    Ok(())
}

fn commit_child_frames(
    journal: &mut dyn CoordinatorJournalV1,
    effects: &mut EffectStackV1,
    terminal: &mut CoordinatorTerminalV1,
) -> Result<(), RuntimeHostSignalV1> {
    if journal.commit_frame().is_err() {
        let _ = journal.revert_frame();
        let _ = effects.revert();
        *terminal = CoordinatorTerminalV1::Fatal;
        return Err(RuntimeHostSignalV1::Abort);
    }
    if effects.commit().is_err() {
        *terminal = CoordinatorTerminalV1::Fatal;
        return Err(RuntimeHostSignalV1::Abort);
    }
    Ok(())
}

pub(super) fn execute_nested_call(
    parent_context: &RuntimeCallContextV1,
    spec: RuntimeCallSpecV1,
    journal: &mut dyn CoordinatorJournalV1,
    meter: &mut WeightMeter,
    effects: &mut EffectStackV1,
    terminal: &mut CoordinatorTerminalV1,
    charges: HostChargeScheduleV1,
    dispatch: &RuntimeDispatchTableV1,
) -> Result<RuntimeCallResultV1, RuntimeHostSignalV1> {
    if *terminal != CoordinatorTerminalV1::Running {
        return Err(RuntimeHostSignalV1::Abort);
    }

    let child_context = match derive_child_context(parent_context, &spec) {
        Ok(context) => context,
        Err(code) => return Ok(RuntimeCallResultV1::trap(code)),
    };
    let Some(factory) = dispatch.factory_for(spec.target()) else {
        return Ok(RuntimeCallResultV1::trap(
            RuntimeTrapCodeV1::InvalidCallTarget,
        ));
    };

    begin_child_frames(journal, effects, terminal)?;

    match transfer_attached_value(journal, parent_context.target(), spec.target(), spec.value()) {
        Ok(()) => {}
        Err(AttachedValueTransferErrorV1::Trap(code)) => {
            rollback_child_frames(journal, effects, terminal)?;
            return Ok(RuntimeCallResultV1::trap(code));
        }
        Err(AttachedValueTransferErrorV1::Fatal) => {
            let _ = rollback_child_frames(journal, effects, terminal);
            *terminal = CoordinatorTerminalV1::Fatal;
            return Err(RuntimeHostSignalV1::Abort);
        }
    }

    let backend_result = {
        let mut backend = factory();
        let mut child_host = CoordinatorHostV1::new_active(
            &child_context,
            journal,
            meter,
            effects,
            terminal,
            charges,
            dispatch,
        );
        backend.execute(&mut child_host)
    };

    if backend_result.is_err() {
        let _ = rollback_child_frames(journal, effects, terminal);
        *terminal = CoordinatorTerminalV1::Fatal;
        return Err(RuntimeHostSignalV1::Abort);
    }
    if *terminal != CoordinatorTerminalV1::Running {
        rollback_child_frames(journal, effects, terminal)?;
        return Err(RuntimeHostSignalV1::Abort);
    }

    let result = backend_result.map_err(|_| RuntimeHostSignalV1::Abort)?;
    if result.validate().is_err() {
        rollback_child_frames(journal, effects, terminal)?;
        return Ok(RuntimeCallResultV1::trap(
            RuntimeTrapCodeV1::ReturnDataTooLarge,
        ));
    }

    let return_bytes = result.return_data().map_or(0, <[u8]>::len);
    if return_bytes != 0 && effects.retain_return_bytes(return_bytes).is_err() {
        rollback_child_frames(journal, effects, terminal)?;
        return Ok(RuntimeCallResultV1::trap(
            RuntimeTrapCodeV1::ReturnDataTooLarge,
        ));
    }

    match &result {
        RuntimeCallResultV1::Success(_) => {
            commit_child_frames(journal, effects, terminal)?;
        }
        RuntimeCallResultV1::Revert(_) => {
            rollback_child_frames(journal, effects, terminal)?;
            if return_bytes != 0 && effects.retain_return_bytes(return_bytes).is_err() {
                *terminal = CoordinatorTerminalV1::Fatal;
                return Err(RuntimeHostSignalV1::Abort);
            }
        }
        RuntimeCallResultV1::Trap(_) => {
            rollback_child_frames(journal, effects, terminal)?;
        }
    }

    Ok(result)
}
