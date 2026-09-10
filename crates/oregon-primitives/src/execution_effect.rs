use crate::execution_receipt::ExecutionReceiptError;
use crate::state_commitment::{CommitmentDomainId, CommitmentSchemeId, MAX_STATE_COMMITMENTS};
use crate::{Hash256, domain_hash};

const STATE_EFFECT_VERSION_V1: u16 = 1;
const STATE_EFFECT_DOMAIN: &[u8] = b"OREGON/EXEC/STATE-EFFECT/V1\0";
const STATE_EFFECT_DESCRIPTOR_BYTES_V1: usize = 68;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateEffectDescriptorV1 {
    domain_id: CommitmentDomainId,
    scheme_id: CommitmentSchemeId,
    old_root: Hash256,
    new_root: Hash256,
}

impl StateEffectDescriptorV1 {
    pub fn new(
        domain_id: CommitmentDomainId,
        scheme_id: CommitmentSchemeId,
        old_root: Hash256,
        new_root: Hash256,
    ) -> Result<Self, ExecutionReceiptError> {
        validate_effect_pair(domain_id, scheme_id)?;
        Ok(Self {
            domain_id,
            scheme_id,
            old_root,
            new_root,
        })
    }

    pub const fn domain_id(&self) -> CommitmentDomainId {
        self.domain_id
    }

    pub const fn scheme_id(&self) -> CommitmentSchemeId {
        self.scheme_id
    }

    pub const fn old_root(&self) -> Hash256 {
        self.old_root
    }

    pub const fn new_root(&self) -> Hash256 {
        self.new_root
    }

    pub fn encode(&self) -> [u8; STATE_EFFECT_DESCRIPTOR_BYTES_V1] {
        let mut bytes = [0u8; STATE_EFFECT_DESCRIPTOR_BYTES_V1];
        bytes[0..2].copy_from_slice(&u16::from(self.domain_id).to_le_bytes());
        bytes[2..4].copy_from_slice(&u16::from(self.scheme_id).to_le_bytes());
        bytes[4..36].copy_from_slice(self.old_root.as_bytes());
        bytes[36..68].copy_from_slice(self.new_root.as_bytes());
        bytes
    }
}

pub fn state_effect_root(
    descriptors: &[StateEffectDescriptorV1],
) -> Result<Hash256, ExecutionReceiptError> {
    if descriptors.len() > MAX_STATE_COMMITMENTS {
        return Err(ExecutionReceiptError::TooManyStateEffects);
    }

    for descriptor in descriptors {
        validate_effect_pair(descriptor.domain_id, descriptor.scheme_id)?;
    }
    for pair in descriptors.windows(2) {
        let left = u16::from(pair[0].domain_id);
        let right = u16::from(pair[1].domain_id);
        if left == right {
            return Err(ExecutionReceiptError::DuplicateEffectDomain);
        }
        if left > right {
            return Err(ExecutionReceiptError::NonCanonicalEffectOrder);
        }
    }

    let mut bytes = Vec::with_capacity(4 + descriptors.len() * STATE_EFFECT_DESCRIPTOR_BYTES_V1);
    bytes.extend_from_slice(&STATE_EFFECT_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&(descriptors.len() as u16).to_le_bytes());
    for descriptor in descriptors {
        bytes.extend_from_slice(&descriptor.encode());
    }
    Ok(domain_hash(STATE_EFFECT_DOMAIN, &bytes))
}

fn validate_effect_pair(
    domain_id: CommitmentDomainId,
    scheme_id: CommitmentSchemeId,
) -> Result<(), ExecutionReceiptError> {
    match domain_id {
        CommitmentDomainId::ExecutionReceipts => {
            Err(ExecutionReceiptError::ReceiptDomainInStateEffects)
        }
        CommitmentDomainId::NativeUtxo => Err(ExecutionReceiptError::InvalidEffectDomain),
        CommitmentDomainId::Evm if scheme_id == CommitmentSchemeId::EvmCommitmentV1 => Ok(()),
        CommitmentDomainId::Wasm
        | CommitmentDomainId::ExecutionAccounting
        | CommitmentDomainId::AsyncOutbox
        | CommitmentDomainId::AsyncConsumed
        | CommitmentDomainId::FeeState
            if scheme_id == CommitmentSchemeId::OregonSmtV1 =>
        {
            Ok(())
        }
        _ => Err(ExecutionReceiptError::InvalidEffectScheme),
    }
}
