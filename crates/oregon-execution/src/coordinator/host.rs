use oregon_primitives::Hash256;
use oregon_primitives::execution_envelope::ExecutionDomain;
use oregon_primitives::execution_event::{
    MAX_EXECUTION_EVENT_DATA_BYTES_V1, MAX_EXECUTION_EVENT_TOPICS_V1,
};
use oregon_runtime::{
    RuntimeCallContextV1, RuntimeCallResultV1, RuntimeCallSpecV1, RuntimeHostSignalV1,
    RuntimeHostV1, RuntimeTrapCodeV1,
};

use crate::{ResourceDomain, WeightMeter};

use super::effects::EffectStackV1;
use super::types::{CoordinatorError, CoordinatorTerminalV1};

const HOST_CHARGE_SCHEDULE_VERSION_V1: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HostChargeScheduleV1Parts {
    pub(super) version: u16,
    pub(super) state_read_base: u64,
    pub(super) state_read_key_byte: u64,
    pub(super) state_read_copy_byte: u64,
    pub(super) state_write_base: u64,
    pub(super) state_write_key_byte: u64,
    pub(super) state_write_value_byte: u64,
    pub(super) state_delete_base: u64,
    pub(super) state_delete_key_byte: u64,
    pub(super) event_base: u64,
    pub(super) event_topic: u64,
    pub(super) event_data_byte: u64,
    pub(super) nested_call_base: u64,
    pub(super) nested_call_input_byte: u64,
    pub(super) nested_call_return_copy_byte: u64,
    pub(super) context_query: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HostChargeScheduleV1 {
    state_read_base: u64,
    state_read_key_byte: u64,
    state_read_copy_byte: u64,
    state_write_base: u64,
    state_write_key_byte: u64,
    state_write_value_byte: u64,
    state_delete_base: u64,
    state_delete_key_byte: u64,
    event_base: u64,
    event_topic: u64,
    event_data_byte: u64,
    nested_call_base: u64,
    nested_call_input_byte: u64,
    nested_call_return_copy_byte: u64,
    context_query: u64,
}

impl HostChargeScheduleV1 {
    pub(super) fn new(parts: HostChargeScheduleV1Parts) -> Result<Self, CoordinatorError> {
        if parts.version != HOST_CHARGE_SCHEDULE_VERSION_V1 {
            return Err(CoordinatorError::UnsupportedHostChargeScheduleVersion(
                parts.version,
            ));
        }

        Ok(Self {
            state_read_base: parts.state_read_base,
            state_read_key_byte: parts.state_read_key_byte,
            state_read_copy_byte: parts.state_read_copy_byte,
            state_write_base: parts.state_write_base,
            state_write_key_byte: parts.state_write_key_byte,
            state_write_value_byte: parts.state_write_value_byte,
            state_delete_base: parts.state_delete_base,
            state_delete_key_byte: parts.state_delete_key_byte,
            event_base: parts.event_base,
            event_topic: parts.event_topic,
            event_data_byte: parts.event_data_byte,
            nested_call_base: parts.nested_call_base,
            nested_call_input_byte: parts.nested_call_input_byte,
            nested_call_return_copy_byte: parts.nested_call_return_copy_byte,
            context_query: parts.context_query,
        })
    }

    fn checked_cost(fixed: u64, terms: &[(u64, usize)]) -> Result<u64, CoordinatorError> {
        let mut total = u128::from(fixed);
        for &(coefficient, count) in terms {
            let count = u128::try_from(count).map_err(|_| CoordinatorError::HostChargeOverflow)?;
            let term = u128::from(coefficient)
                .checked_mul(count)
                .ok_or(CoordinatorError::HostChargeOverflow)?;
            total = total
                .checked_add(term)
                .ok_or(CoordinatorError::HostChargeOverflow)?;
        }
        u64::try_from(total).map_err(|_| CoordinatorError::HostChargeOverflow)
    }

    fn state_read_pre_cost(self, key_len: usize) -> Result<u64, CoordinatorError> {
        Self::checked_cost(
            self.state_read_base,
            &[
                (self.state_read_key_byte, key_len),
                (self.state_read_copy_byte, 0),
            ],
        )
    }

    fn state_write_cost(self, key_len: usize, value_len: usize) -> Result<u64, CoordinatorError> {
        Self::checked_cost(
            self.state_write_base,
            &[
                (self.state_write_key_byte, key_len),
                (self.state_write_value_byte, value_len),
            ],
        )
    }

    fn state_delete_cost(self, key_len: usize) -> Result<u64, CoordinatorError> {
        Self::checked_cost(
            self.state_delete_base,
            &[(self.state_delete_key_byte, key_len)],
        )
    }

    fn event_cost(self, topic_count: usize, data_len: usize) -> Result<u64, CoordinatorError> {
        Self::checked_cost(
            self.event_base,
            &[
                (self.event_topic, topic_count),
                (self.event_data_byte, data_len),
            ],
        )
    }

    fn nested_call_pre_cost(self, input_len: usize) -> Result<u64, CoordinatorError> {
        Self::checked_cost(
            self.nested_call_base,
            &[
                (self.nested_call_input_byte, input_len),
                (self.nested_call_return_copy_byte, 0),
            ],
        )
    }
}

pub(super) struct CoordinatorHostV1<'a> {
    context: &'a RuntimeCallContextV1,
    meter: &'a mut WeightMeter,
    effects: &'a mut EffectStackV1,
    terminal: &'a mut CoordinatorTerminalV1,
    charges: HostChargeScheduleV1,
}

impl<'a> CoordinatorHostV1<'a> {
    pub(super) fn new(
        context: &'a RuntimeCallContextV1,
        meter: &'a mut WeightMeter,
        effects: &'a mut EffectStackV1,
        terminal: &'a mut CoordinatorTerminalV1,
        charges: HostChargeScheduleV1,
    ) -> Self {
        Self {
            context,
            meter,
            effects,
            terminal,
            charges,
        }
    }

    pub(super) fn charge_context_query(&mut self) -> Result<(), RuntimeHostSignalV1> {
        self.charge_common_weight(self.charges.context_query)
    }

    fn ensure_running(&self) -> Result<(), RuntimeHostSignalV1> {
        if *self.terminal == CoordinatorTerminalV1::Running {
            Ok(())
        } else {
            Err(RuntimeHostSignalV1::Abort)
        }
    }

    fn latch_fatal(&mut self) -> RuntimeHostSignalV1 {
        *self.terminal = CoordinatorTerminalV1::Fatal;
        RuntimeHostSignalV1::Abort
    }

    fn checked_host_cost(
        &mut self,
        cost: Result<u64, CoordinatorError>,
    ) -> Result<u64, RuntimeHostSignalV1> {
        match cost {
            Ok(value) => Ok(value),
            Err(CoordinatorError::HostChargeOverflow) => Err(self.latch_fatal()),
            Err(_) => Err(self.latch_fatal()),
        }
    }

    fn charge_common_weight(&mut self, weight: u64) -> Result<(), RuntimeHostSignalV1> {
        self.ensure_running()?;
        if self.meter.charge_common(weight).is_err() {
            *self.terminal = CoordinatorTerminalV1::ResourceExhausted;
            return Err(RuntimeHostSignalV1::Abort);
        }
        Ok(())
    }

    fn charge_vm_weight(&mut self, units: u64) -> Result<(), RuntimeHostSignalV1> {
        self.ensure_running()?;
        let domain = match self.context.execution_domain() {
            ExecutionDomain::Evm => ResourceDomain::Evm,
            ExecutionDomain::Wasm => ResourceDomain::Wasm,
            ExecutionDomain::Native | ExecutionDomain::System => return Err(self.latch_fatal()),
        };
        if self.meter.charge(domain, units).is_err() {
            *self.terminal = CoordinatorTerminalV1::ResourceExhausted;
            return Err(RuntimeHostSignalV1::Abort);
        }
        Ok(())
    }

    fn map_effect_error(&mut self, error: CoordinatorError) -> RuntimeHostSignalV1 {
        match error {
            CoordinatorError::EventLimitExceeded
            | CoordinatorError::EventDataTooLarge
            | CoordinatorError::EventTopicsTooLarge
            | CoordinatorError::EffectBytesLimitExceeded => {
                RuntimeHostSignalV1::Trap(RuntimeTrapCodeV1::EventLimitExceeded)
            }
            _ => self.latch_fatal(),
        }
    }
}

impl RuntimeHostV1 for CoordinatorHostV1<'_> {
    fn context(&self) -> &RuntimeCallContextV1 {
        self.context
    }

    fn state_get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>, RuntimeHostSignalV1> {
        let cost = self.checked_host_cost(self.charges.state_read_pre_cost(key.len()))?;
        self.charge_common_weight(cost)?;
        Err(RuntimeHostSignalV1::Trap(
            RuntimeTrapCodeV1::UnsupportedOperation,
        ))
    }

    fn state_put(&mut self, key: &[u8], value: &[u8]) -> Result<(), RuntimeHostSignalV1> {
        let cost = self.checked_host_cost(self.charges.state_write_cost(key.len(), value.len()))?;
        self.charge_common_weight(cost)?;
        Err(RuntimeHostSignalV1::Trap(
            RuntimeTrapCodeV1::UnsupportedOperation,
        ))
    }

    fn state_delete(&mut self, key: &[u8]) -> Result<(), RuntimeHostSignalV1> {
        let cost = self.checked_host_cost(self.charges.state_delete_cost(key.len()))?;
        self.charge_common_weight(cost)?;
        Err(RuntimeHostSignalV1::Trap(
            RuntimeTrapCodeV1::UnsupportedOperation,
        ))
    }

    fn emit_event(&mut self, topics: &[Hash256], data: &[u8]) -> Result<(), RuntimeHostSignalV1> {
        self.ensure_running()?;
        if topics.len() > MAX_EXECUTION_EVENT_TOPICS_V1
            || data.len() > MAX_EXECUTION_EVENT_DATA_BYTES_V1
        {
            return Err(RuntimeHostSignalV1::Trap(
                RuntimeTrapCodeV1::EventLimitExceeded,
            ));
        }

        let cost = self.checked_host_cost(self.charges.event_cost(topics.len(), data.len()))?;
        self.charge_common_weight(cost)?;

        if let Err(error) = self
            .effects
            .push_event_parts(self.context.target(), topics, data)
        {
            return Err(self.map_effect_error(error));
        }
        Ok(())
    }

    fn call(
        &mut self,
        spec: RuntimeCallSpecV1,
    ) -> Result<RuntimeCallResultV1, RuntimeHostSignalV1> {
        let cost = self.checked_host_cost(self.charges.nested_call_pre_cost(spec.input().len()))?;
        self.charge_common_weight(cost)?;
        Ok(RuntimeCallResultV1::trap(
            RuntimeTrapCodeV1::UnsupportedOperation,
        ))
    }

    fn charge_vm_units(&mut self, units: u64) -> Result<(), RuntimeHostSignalV1> {
        self.charge_vm_weight(units)
    }

    fn charge_common(&mut self, units: u64) -> Result<(), RuntimeHostSignalV1> {
        self.charge_common_weight(units)
    }
}
