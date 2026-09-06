use oregon_primitives::Hash256;
use thiserror::Error;

use crate::{RuntimeCallContextV1, RuntimeCallResultV1, RuntimeCallSpecV1, RuntimeTrapCodeV1};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeHostSignalV1 {
    Trap(RuntimeTrapCodeV1),
    Abort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RuntimeBackendFailureV1 {
    #[error("runtime backend failed outside deterministic result semantics")]
    Fatal,
}

pub trait RuntimeHostV1 {
    fn context(&self) -> &RuntimeCallContextV1;

    fn state_get(&mut self, key: &[u8]) -> Result<Option<Vec<u8>>, RuntimeHostSignalV1>;

    fn state_put(&mut self, key: &[u8], value: &[u8]) -> Result<(), RuntimeHostSignalV1>;

    fn state_delete(&mut self, key: &[u8]) -> Result<(), RuntimeHostSignalV1>;

    fn emit_event(&mut self, topics: &[Hash256], data: &[u8]) -> Result<(), RuntimeHostSignalV1>;

    fn call(&mut self, spec: RuntimeCallSpecV1)
    -> Result<RuntimeCallResultV1, RuntimeHostSignalV1>;

    fn charge_vm_units(&mut self, units: u64) -> Result<(), RuntimeHostSignalV1>;

    fn charge_common(&mut self, units: u64) -> Result<(), RuntimeHostSignalV1>;
}

pub trait RuntimeBackendV1 {
    fn execute(
        &mut self,
        host: &mut dyn RuntimeHostV1,
    ) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1>;
}
