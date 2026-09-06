//! Inactive typed execution-address primitives from Execution Architecture V1.
//!
//! These identities do not activate VM execution, change native UTXO ownership,
//! or confer authority. In particular, a system kind is not authorization.

use thiserror::Error;

/// The closed V1 internal execution-address namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ExecutionAddressKind {
    Evm = 0x01,
    Wasm = 0x02,
    Oregon = 0x03,
    System = 0x04,
}

impl TryFrom<u8> for ExecutionAddressKind {
    type Error = ExecutionAddressError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Self::Evm),
            0x02 => Ok(Self::Wasm),
            0x03 => Ok(Self::Oregon),
            0x04 => Ok(Self::System),
            _ => Err(ExecutionAddressError::UnknownKind(value)),
        }
    }
}

/// A canonical 33-byte identity: one kind byte followed by 32 payload bytes.
///
/// Private fields preserve validation across every public construction path.
/// Equality and hashing include the kind, so distinct namespaces cannot alias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExecutionAddress {
    kind: ExecutionAddressKind,
    payload: [u8; 32],
}

impl ExecutionAddress {
    /// Validate a typed payload without normalizing malformed EVM padding.
    pub fn new(
        kind: ExecutionAddressKind,
        payload: [u8; 32],
    ) -> Result<Self, ExecutionAddressError> {
        if kind == ExecutionAddressKind::Evm && payload[..12].iter().any(|&byte| byte != 0) {
            return Err(ExecutionAddressError::NonCanonicalEvmPadding);
        }
        Ok(Self { kind, payload })
    }

    /// Embed an Ethereum address with exactly twelve leading zero bytes.
    pub fn from_evm(address: [u8; 20]) -> Self {
        let mut payload = [0; 32];
        payload[12..].copy_from_slice(&address);
        Self {
            kind: ExecutionAddressKind::Evm,
            payload,
        }
    }

    /// Decode one complete identity, rejecting unknown kinds and extra bytes.
    /// The exact-width check precedes any access and allocates no heap memory.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, ExecutionAddressError> {
        let bytes: &[u8; 33] = bytes
            .try_into()
            .map_err(|_| ExecutionAddressError::InvalidLength(bytes.len()))?;
        let kind = ExecutionAddressKind::try_from(bytes[0])?;
        let mut payload = [0; 32];
        payload.copy_from_slice(&bytes[1..]);
        Self::new(kind, payload)
    }

    pub const fn kind(&self) -> ExecutionAddressKind {
        self.kind
    }

    pub const fn payload(&self) -> &[u8; 32] {
        &self.payload
    }

    pub fn to_bytes(&self) -> [u8; 33] {
        let mut bytes = [0; 33];
        bytes[0] = self.kind as u8;
        bytes[1..].copy_from_slice(&self.payload);
        bytes
    }

    /// Return the external EVM identity only for the EVM namespace.
    pub fn evm_address(&self) -> Option<[u8; 20]> {
        if self.kind != ExecutionAddressKind::Evm {
            return None;
        }
        let mut address = [0; 20];
        address.copy_from_slice(&self.payload[12..]);
        Some(address)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExecutionAddressError {
    #[error("invalid execution address length: expected 33, got {0}")]
    InvalidLength(usize),
    #[error("unknown execution address kind {0}")]
    UnknownKind(u8),
    #[error("EVM execution address must have twelve leading zero payload bytes")]
    NonCanonicalEvmPadding,
}
