use thiserror::Error;

use crate::{Hash256, PrimitiveError, domain_hash};

const AGGREGATE_VERSION: u16 = 1;
const DESCRIPTOR_BYTES: usize = 36;
const AGGREGATE_DOMAIN: &[u8] = b"OREGON/STATE/AGGREGATE/V1\0";

pub const MAX_STATE_COMMITMENTS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum CommitmentDomainId {
    NativeUtxo = 0x0001,
    Evm = 0x0010,
    Wasm = 0x0011,
    ExecutionAccounting = 0x0020,
    ExecutionReceipts = 0x0030,
    AsyncOutbox = 0x0040,
    AsyncConsumed = 0x0041,
    FeeState = 0x0050,
}

impl From<CommitmentDomainId> for u16 {
    fn from(value: CommitmentDomainId) -> Self {
        value as u16
    }
}

impl TryFrom<u16> for CommitmentDomainId {
    type Error = StateCommitmentError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x0001 => Ok(Self::NativeUtxo),
            0x0010 => Ok(Self::Evm),
            0x0011 => Ok(Self::Wasm),
            0x0020 => Ok(Self::ExecutionAccounting),
            0x0030 => Ok(Self::ExecutionReceipts),
            0x0040 => Ok(Self::AsyncOutbox),
            0x0041 => Ok(Self::AsyncConsumed),
            0x0050 => Ok(Self::FeeState),
            _ => Err(StateCommitmentError::UnknownCommitmentDomain(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum CommitmentSchemeId {
    OregonSmtV1 = 0x0001,
    EvmCommitmentV1 = 0x0100,
}

impl From<CommitmentSchemeId> for u16 {
    fn from(value: CommitmentSchemeId) -> Self {
        value as u16
    }
}

impl TryFrom<u16> for CommitmentSchemeId {
    type Error = StateCommitmentError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x0001 => Ok(Self::OregonSmtV1),
            0x0100 => Ok(Self::EvmCommitmentV1),
            _ => Err(StateCommitmentError::UnknownCommitmentScheme(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateCommitmentDescriptor {
    domain_id: CommitmentDomainId,
    scheme_id: CommitmentSchemeId,
    root: Hash256,
}

impl StateCommitmentDescriptor {
    pub const fn new(
        domain_id: CommitmentDomainId,
        scheme_id: CommitmentSchemeId,
        root: Hash256,
    ) -> Self {
        Self {
            domain_id,
            scheme_id,
            root,
        }
    }

    pub const fn domain_id(&self) -> CommitmentDomainId {
        self.domain_id
    }

    pub const fn scheme_id(&self) -> CommitmentSchemeId {
        self.scheme_id
    }

    pub const fn root(&self) -> Hash256 {
        self.root
    }

    pub fn encode(&self) -> [u8; DESCRIPTOR_BYTES] {
        let mut bytes = [0u8; DESCRIPTOR_BYTES];
        bytes[0..2].copy_from_slice(&u16::from(self.domain_id).to_le_bytes());
        bytes[2..4].copy_from_slice(&u16::from(self.scheme_id).to_le_bytes());
        bytes[4..].copy_from_slice(self.root.as_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, StateCommitmentError> {
        if bytes.len() != DESCRIPTOR_BYTES {
            return Err(StateCommitmentError::InvalidDescriptorLength(bytes.len()));
        }

        let domain_id = CommitmentDomainId::try_from(u16::from_le_bytes([bytes[0], bytes[1]]))?;
        let scheme_id = CommitmentSchemeId::try_from(u16::from_le_bytes([bytes[2], bytes[3]]))?;
        let root = Hash256::from_slice(&bytes[4..])?;
        Ok(Self::new(domain_id, scheme_id, root))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateCommitmentSetV1 {
    descriptors: Vec<StateCommitmentDescriptor>,
}

impl StateCommitmentSetV1 {
    pub fn new(descriptors: Vec<StateCommitmentDescriptor>) -> Result<Self, StateCommitmentError> {
        if descriptors.is_empty() {
            return Err(StateCommitmentError::EmptyCommitmentSet);
        }
        if descriptors.len() > MAX_STATE_COMMITMENTS {
            return Err(StateCommitmentError::TooManyCommitments(descriptors.len()));
        }

        for pair in descriptors.windows(2) {
            let left = u16::from(pair[0].domain_id);
            let right = u16::from(pair[1].domain_id);
            if left == right {
                return Err(StateCommitmentError::DuplicateCommitmentDomain(
                    pair[0].domain_id,
                ));
            }
            if left > right {
                return Err(StateCommitmentError::NonCanonicalDomainOrder);
            }
        }

        Ok(Self { descriptors })
    }

    pub fn descriptors(&self) -> &[StateCommitmentDescriptor] {
        &self.descriptors
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(2 + self.descriptors.len() * DESCRIPTOR_BYTES);
        bytes.extend_from_slice(&AGGREGATE_VERSION.to_le_bytes());
        for descriptor in &self.descriptors {
            bytes.extend_from_slice(&descriptor.encode());
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, StateCommitmentError> {
        if bytes.len() < 2 {
            return Err(StateCommitmentError::InvalidAggregateLength(bytes.len()));
        }

        let version = u16::from_le_bytes([bytes[0], bytes[1]]);
        if version != AGGREGATE_VERSION {
            return Err(StateCommitmentError::UnsupportedAggregateVersion(version));
        }

        let descriptor_bytes = bytes.len() - 2;
        if descriptor_bytes == 0 {
            return Err(StateCommitmentError::EmptyCommitmentSet);
        }
        if descriptor_bytes % DESCRIPTOR_BYTES != 0 {
            return Err(StateCommitmentError::InvalidAggregateLength(bytes.len()));
        }

        let count = descriptor_bytes / DESCRIPTOR_BYTES;
        if count > MAX_STATE_COMMITMENTS {
            return Err(StateCommitmentError::TooManyCommitments(count));
        }

        let mut descriptors = Vec::new();
        for chunk in bytes[2..].chunks_exact(DESCRIPTOR_BYTES) {
            descriptors.push(StateCommitmentDescriptor::decode(chunk)?);
        }
        Self::new(descriptors)
    }

    pub fn root(&self) -> Hash256 {
        domain_hash(AGGREGATE_DOMAIN, &self.encode())
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StateCommitmentError {
    #[error("unknown commitment domain id {0:#06x}")]
    UnknownCommitmentDomain(u16),
    #[error("unknown commitment scheme id {0:#06x}")]
    UnknownCommitmentScheme(u16),
    #[error("state commitment descriptor must be exactly 36 bytes, got {0}")]
    InvalidDescriptorLength(usize),
    #[error("unsupported state commitment aggregate version {0}")]
    UnsupportedAggregateVersion(u16),
    #[error("invalid state commitment aggregate length {0}")]
    InvalidAggregateLength(usize),
    #[error("state commitment set must contain at least one descriptor")]
    EmptyCommitmentSet,
    #[error("state commitment set has {0} descriptors, exceeding the limit")]
    TooManyCommitments(usize),
    #[error("state commitment descriptors are not in strictly increasing domain order")]
    NonCanonicalDomainOrder,
    #[error("duplicate state commitment domain {0:?}")]
    DuplicateCommitmentDomain(CommitmentDomainId),
    #[error(transparent)]
    Primitive(#[from] PrimitiveError),
}
