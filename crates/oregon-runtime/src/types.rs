use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
use oregon_primitives::execution_envelope::ExecutionDomain;
use oregon_primitives::Hash256;
use thiserror::Error;

pub const RUNTIME_ABI_VERSION_V1: u16 = 1;
pub const MAX_RUNTIME_CALL_DEPTH: u16 = 64;
pub const MAX_RUNTIME_CALL_INPUT_BYTES: usize = 262_144;
pub const MAX_RUNTIME_RETURN_DATA_BYTES: usize = 262_144;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RuntimeAbiError {
    #[error("runtime call target is not an activated EVM or WASM namespace")]
    InvalidCallTarget,
    #[error("runtime call input exceeds the V1 structural ceiling")]
    CallInputTooLarge,
    #[error("runtime call depth must be within the V1 structural ceiling")]
    InvalidCallDepth,
    #[error("runtime call context target does not match its execution domain")]
    ContextDomainTargetMismatch,
    #[error("runtime return data exceeds the V1 structural ceiling")]
    ReturnDataTooLarge,
    #[error("unknown runtime trap code {0:#06x}")]
    UnknownTrapCode(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCallSpecV1 {
    target: ExecutionAddress,
    value: u64,
    read_only: bool,
    input: Vec<u8>,
}

impl RuntimeCallSpecV1 {
    pub fn new(
        target: ExecutionAddress,
        value: u64,
        read_only: bool,
        input: Vec<u8>,
    ) -> Result<Self, RuntimeAbiError> {
        if input.len() > MAX_RUNTIME_CALL_INPUT_BYTES {
            return Err(RuntimeAbiError::CallInputTooLarge);
        }
        if !matches!(
            target.kind(),
            ExecutionAddressKind::Evm | ExecutionAddressKind::Wasm
        ) {
            return Err(RuntimeAbiError::InvalidCallTarget);
        }
        Ok(Self {
            target,
            value,
            read_only,
            input,
        })
    }

    pub const fn target(&self) -> ExecutionAddress {
        self.target
    }

    pub const fn value(&self) -> u64 {
        self.value
    }

    pub const fn read_only(&self) -> bool {
        self.read_only
    }

    pub fn input(&self) -> &[u8] {
        &self.input
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCallContextV1Parts {
    pub chain_id: u64,
    pub height: u64,
    pub parent_block_hash: Hash256,
    pub txid: Hash256,
    pub principal: ExecutionAddress,
    pub caller: ExecutionAddress,
    pub target: ExecutionAddress,
    pub execution_domain: ExecutionDomain,
    pub depth: u16,
    pub read_only: bool,
    pub transferred_value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCallContextV1 {
    parts: RuntimeCallContextV1Parts,
}

impl RuntimeCallContextV1 {
    pub fn from_trusted_parts(parts: RuntimeCallContextV1Parts) -> Result<Self, RuntimeAbiError> {
        if !(1..=MAX_RUNTIME_CALL_DEPTH).contains(&parts.depth) {
            return Err(RuntimeAbiError::InvalidCallDepth);
        }

        let target_matches_domain = matches!(
            (parts.execution_domain, parts.target.kind()),
            (ExecutionDomain::Evm, ExecutionAddressKind::Evm)
                | (ExecutionDomain::Wasm, ExecutionAddressKind::Wasm)
        );
        if !target_matches_domain {
            return Err(RuntimeAbiError::ContextDomainTargetMismatch);
        }

        Ok(Self { parts })
    }

    pub const fn chain_id(&self) -> u64 {
        self.parts.chain_id
    }

    pub const fn height(&self) -> u64 {
        self.parts.height
    }

    pub const fn parent_block_hash(&self) -> Hash256 {
        self.parts.parent_block_hash
    }

    pub const fn txid(&self) -> Hash256 {
        self.parts.txid
    }

    pub const fn principal(&self) -> ExecutionAddress {
        self.parts.principal
    }

    pub const fn caller(&self) -> ExecutionAddress {
        self.parts.caller
    }

    pub const fn target(&self) -> ExecutionAddress {
        self.parts.target
    }

    pub const fn execution_domain(&self) -> ExecutionDomain {
        self.parts.execution_domain
    }

    pub const fn depth(&self) -> u16 {
        self.parts.depth
    }

    pub const fn read_only(&self) -> bool {
        self.parts.read_only
    }

    pub const fn transferred_value(&self) -> u64 {
        self.parts.transferred_value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum RuntimeTrapCodeV1 {
    BackendDeterministic = 0x0001,
    InvalidCallTarget = 0x0002,
    StateAccessDenied = 0x0003,
    ReadOnlyViolation = 0x0004,
    CallDepthExceeded = 0x0005,
    CallInputTooLarge = 0x0006,
    ReturnDataTooLarge = 0x0007,
    EventLimitExceeded = 0x0008,
    AttachedValueTransferFailed = 0x0009,
    UnsupportedOperation = 0x000a,
    InvalidHostInput = 0x000b,
}

impl TryFrom<u16> for RuntimeTrapCodeV1 {
    type Error = RuntimeAbiError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x0001 => Ok(Self::BackendDeterministic),
            0x0002 => Ok(Self::InvalidCallTarget),
            0x0003 => Ok(Self::StateAccessDenied),
            0x0004 => Ok(Self::ReadOnlyViolation),
            0x0005 => Ok(Self::CallDepthExceeded),
            0x0006 => Ok(Self::CallInputTooLarge),
            0x0007 => Ok(Self::ReturnDataTooLarge),
            0x0008 => Ok(Self::EventLimitExceeded),
            0x0009 => Ok(Self::AttachedValueTransferFailed),
            0x000a => Ok(Self::UnsupportedOperation),
            0x000b => Ok(Self::InvalidHostInput),
            _ => Err(RuntimeAbiError::UnknownTrapCode(value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCallResultV1 {
    Success(Vec<u8>),
    Revert(Vec<u8>),
    Trap(RuntimeTrapCodeV1),
}

impl RuntimeCallResultV1 {
    pub fn success(return_data: Vec<u8>) -> Result<Self, RuntimeAbiError> {
        let result = Self::Success(return_data);
        result.validate()?;
        Ok(result)
    }

    pub fn revert(return_data: Vec<u8>) -> Result<Self, RuntimeAbiError> {
        let result = Self::Revert(return_data);
        result.validate()?;
        Ok(result)
    }

    pub const fn trap(code: RuntimeTrapCodeV1) -> Self {
        Self::Trap(code)
    }

    pub fn validate(&self) -> Result<(), RuntimeAbiError> {
        if matches!(
            self,
            Self::Success(data) | Self::Revert(data) if data.len() > MAX_RUNTIME_RETURN_DATA_BYTES
        ) {
            return Err(RuntimeAbiError::ReturnDataTooLarge);
        }
        Ok(())
    }

    pub fn return_data(&self) -> Option<&[u8]> {
        match self {
            Self::Success(data) | Self::Revert(data) => Some(data),
            Self::Trap(_) => None,
        }
    }

    pub const fn trap_code(&self) -> Option<RuntimeTrapCodeV1> {
        match self {
            Self::Trap(code) => Some(*code),
            Self::Success(_) | Self::Revert(_) => None,
        }
    }
}
