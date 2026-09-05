//! Inactive universal execution-envelope and authorization wire primitives.
//!
//! This module implements the byte-exact V1 outer framing selected by Oregon's
//! execution architecture. It does not activate the envelope in consensus,
//! mempool, block, RPC, wallet, EVM, WASM, or native UTXO execution paths.

use thiserror::Error;

use crate::execution_address::{ExecutionAddress, ExecutionAddressError};
use crate::hash::domain_hash;
use crate::{Decoder, Hash256, PrimitiveError, write_varint};

pub const MAX_ENVELOPE_BYTES: usize = 2_097_152;
pub const MAX_DOMAIN_PAYLOAD_BYTES: usize = 1_048_576;
pub const MAX_ACCESS_HINT_BYTES: usize = 262_144;
pub const MAX_AUTH_PROOFS: usize = 2;
pub const MAX_AUTH_PROOF_BYTES: usize = 4_096;

const ENVELOPE_VERSION_V1: u16 = 1;
const SIGNING_DOMAIN: &[u8] = b"OREGON/ENVELOPE/SIGN/V1\0";
const TXID_DOMAIN: &[u8] = b"OREGON/ENVELOPE/TXID/V1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ExecutionDomain {
    Native = 0x10,
    Evm = 0x11,
    Wasm = 0x12,
    System = 0x13,
}

impl TryFrom<u8> for ExecutionDomain {
    type Error = ExecutionEnvelopeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x10 => Ok(Self::Native),
            0x11 => Ok(Self::Evm),
            0x12 => Ok(Self::Wasm),
            0x13 => Ok(Self::System),
            _ => Err(ExecutionEnvelopeError::UnknownDomain(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum AuthorizationScope {
    Principal = 0x01,
    FeePayer = 0x02,
}

impl TryFrom<u8> for AuthorizationScope {
    type Error = ExecutionEnvelopeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Self::Principal),
            0x02 => Ok(Self::FeePayer),
            _ => Err(ExecutionEnvelopeError::UnknownAuthorizationScope(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum AuthorizationScheme {
    OregonSchnorrV1 = 0x0001,
    EthereumEcdsaV1 = 0x0002,
    OregonThresholdV1 = 0x0003,
}

impl TryFrom<u16> for AuthorizationScheme {
    type Error = ExecutionEnvelopeError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x0001 => Ok(Self::OregonSchnorrV1),
            0x0002 => Ok(Self::EthereumEcdsaV1),
            0x0003 => Ok(Self::OregonThresholdV1),
            _ => Err(ExecutionEnvelopeError::UnknownAuthorizationScheme(value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationProof {
    scope: AuthorizationScope,
    scheme: AuthorizationScheme,
    proof: Vec<u8>,
}

impl AuthorizationProof {
    pub fn new(
        scope: AuthorizationScope,
        scheme: AuthorizationScheme,
        proof: Vec<u8>,
    ) -> Result<Self, ExecutionEnvelopeError> {
        let valid_length = match scheme {
            AuthorizationScheme::OregonSchnorrV1 => proof.len() == 96,
            AuthorizationScheme::EthereumEcdsaV1 => proof.len() == 65,
            AuthorizationScheme::OregonThresholdV1 => {
                (1..=MAX_AUTH_PROOF_BYTES).contains(&proof.len())
            }
        };
        if !valid_length {
            return Err(ExecutionEnvelopeError::InvalidAuthorizationProofLength);
        }
        Ok(Self {
            scope,
            scheme,
            proof,
        })
    }

    pub const fn scope(&self) -> AuthorizationScope {
        self.scope
    }

    pub const fn scheme(&self) -> AuthorizationScheme {
        self.scheme
    }

    pub fn proof(&self) -> &[u8] {
        &self.proof
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeCaps {
    max_fee_per_weight: u64,
    max_priority_fee_per_weight: u64,
    max_weight: u64,
}

impl FeeCaps {
    pub fn new(
        max_fee_per_weight: u64,
        max_priority_fee_per_weight: u64,
        max_weight: u64,
    ) -> Result<Self, ExecutionEnvelopeError> {
        if max_weight == 0 {
            return Err(ExecutionEnvelopeError::ZeroMaxWeight);
        }
        if max_priority_fee_per_weight > max_fee_per_weight {
            return Err(ExecutionEnvelopeError::PriorityFeeExceedsMaxFee);
        }
        Ok(Self {
            max_fee_per_weight,
            max_priority_fee_per_weight,
            max_weight,
        })
    }

    pub const fn max_fee_per_weight(&self) -> u64 {
        self.max_fee_per_weight
    }

    pub const fn max_priority_fee_per_weight(&self) -> u64 {
        self.max_priority_fee_per_weight
    }

    pub const fn max_weight(&self) -> u64 {
        self.max_weight
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionEnvelopeV1Parts {
    pub chain_id: u64,
    pub execution_domain: ExecutionDomain,
    pub valid_after_height: u64,
    pub valid_until_height: u64,
    pub principal: ExecutionAddress,
    pub fee_payer: Option<ExecutionAddress>,
    pub fee_caps: FeeCaps,
    pub authorizations: Vec<AuthorizationProof>,
    pub domain_payload: Vec<u8>,
    pub access_hints: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionEnvelopeV1 {
    chain_id: u64,
    execution_domain: ExecutionDomain,
    valid_after_height: u64,
    valid_until_height: u64,
    principal: ExecutionAddress,
    fee_payer: Option<ExecutionAddress>,
    fee_caps: FeeCaps,
    authorizations: Vec<AuthorizationProof>,
    domain_payload: Vec<u8>,
    access_hints: Option<Vec<u8>>,
}

impl ExecutionEnvelopeV1 {
    pub fn new(parts: ExecutionEnvelopeV1Parts) -> Result<Self, ExecutionEnvelopeError> {
        if parts.valid_after_height > parts.valid_until_height {
            return Err(ExecutionEnvelopeError::InvalidHeightWindow);
        }
        if parts.fee_payer == Some(parts.principal) {
            return Err(ExecutionEnvelopeError::FeePayerEqualsPrincipal);
        }
        if parts.domain_payload.len() > MAX_DOMAIN_PAYLOAD_BYTES {
            return Err(ExecutionEnvelopeError::DomainPayloadTooLarge);
        }
        if let Some(hints) = &parts.access_hints {
            if hints.is_empty() {
                return Err(ExecutionEnvelopeError::EmptyAccessHints);
            }
            if hints.len() > MAX_ACCESS_HINT_BYTES {
                return Err(ExecutionEnvelopeError::AccessHintsTooLarge);
            }
        }
        validate_authorizations(
            parts.execution_domain,
            parts.valid_after_height,
            parts.valid_until_height,
            parts.fee_payer.is_some(),
            &parts.authorizations,
        )?;

        Ok(Self {
            chain_id: parts.chain_id,
            execution_domain: parts.execution_domain,
            valid_after_height: parts.valid_after_height,
            valid_until_height: parts.valid_until_height,
            principal: parts.principal,
            fee_payer: parts.fee_payer,
            fee_caps: parts.fee_caps,
            authorizations: parts.authorizations,
            domain_payload: parts.domain_payload,
            access_hints: parts.access_hints,
        })
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ExecutionEnvelopeError> {
        if bytes.len() > MAX_ENVELOPE_BYTES {
            return Err(ExecutionEnvelopeError::EnvelopeTooLarge);
        }

        let mut decoder = Decoder::new(bytes);
        let version = decoder.read_u16()?;
        if version != ENVELOPE_VERSION_V1 {
            return Err(ExecutionEnvelopeError::UnsupportedVersion(version));
        }
        let chain_id = decoder.read_u64()?;
        let execution_domain = ExecutionDomain::try_from(read_u8(&mut decoder)?)?;
        let valid_after_height = decoder.read_u64()?;
        let valid_until_height = decoder.read_u64()?;
        let principal = ExecutionAddress::from_slice(decoder.read_bytes(33)?)?;

        let fee_payer = match read_u8(&mut decoder)? {
            0 => None,
            1 => Some(ExecutionAddress::from_slice(decoder.read_bytes(33)?)?),
            other => return Err(ExecutionEnvelopeError::InvalidOptionFlag(other)),
        };

        let fee_caps = FeeCaps::new(
            decoder.read_u64()?,
            decoder.read_u64()?,
            decoder.read_u64()?,
        )?;

        let authorization_count = decoder.read_len(MAX_AUTH_PROOFS)?;
        let mut authorizations = Vec::new();
        for _ in 0..authorization_count {
            let scope = AuthorizationScope::try_from(read_u8(&mut decoder)?)?;
            let scheme = AuthorizationScheme::try_from(decoder.read_u16()?)?;
            let proof_len = decoder.read_len(MAX_AUTH_PROOF_BYTES)?;
            let proof = decoder.read_bytes(proof_len)?.to_vec();
            authorizations.push(AuthorizationProof::new(scope, scheme, proof)?);
        }

        let payload_len = decoder.read_len(MAX_DOMAIN_PAYLOAD_BYTES)?;
        let domain_payload = decoder.read_bytes(payload_len)?.to_vec();

        let access_hints = match read_u8(&mut decoder)? {
            0 => None,
            1 => {
                let hint_len = decoder.read_len(MAX_ACCESS_HINT_BYTES)?;
                Some(decoder.read_bytes(hint_len)?.to_vec())
            }
            other => return Err(ExecutionEnvelopeError::InvalidOptionFlag(other)),
        };

        decoder.finish()?;

        Self::new(ExecutionEnvelopeV1Parts {
            chain_id,
            execution_domain,
            valid_after_height,
            valid_until_height,
            principal,
            fee_payer,
            fee_caps,
            authorizations,
            domain_payload,
            access_hints,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.encode_common_prefix(&mut bytes);
        write_varint(self.authorizations.len() as u64, &mut bytes);
        for authorization in &self.authorizations {
            bytes.push(authorization.scope as u8);
            bytes.extend_from_slice(&(authorization.scheme as u16).to_le_bytes());
            write_varint(authorization.proof.len() as u64, &mut bytes);
            bytes.extend_from_slice(&authorization.proof);
        }
        self.encode_payload_and_hints(&mut bytes);
        bytes
    }

    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.encode_common_prefix(&mut bytes);
        write_varint(self.authorizations.len() as u64, &mut bytes);
        for authorization in &self.authorizations {
            bytes.push(authorization.scope as u8);
            bytes.extend_from_slice(&(authorization.scheme as u16).to_le_bytes());
        }
        self.encode_payload_and_hints(&mut bytes);
        bytes
    }

    pub fn signing_hash(&self) -> Hash256 {
        domain_hash(SIGNING_DOMAIN, &self.signing_bytes())
    }

    pub fn txid(&self) -> Hash256 {
        domain_hash(TXID_DOMAIN, &self.encode())
    }

    pub const fn chain_id(&self) -> u64 {
        self.chain_id
    }

    pub const fn execution_domain(&self) -> ExecutionDomain {
        self.execution_domain
    }

    pub const fn valid_after_height(&self) -> u64 {
        self.valid_after_height
    }

    pub const fn valid_until_height(&self) -> u64 {
        self.valid_until_height
    }

    pub const fn principal(&self) -> &ExecutionAddress {
        &self.principal
    }

    pub const fn fee_payer(&self) -> Option<&ExecutionAddress> {
        self.fee_payer.as_ref()
    }

    pub const fn fee_caps(&self) -> FeeCaps {
        self.fee_caps
    }

    pub fn authorizations(&self) -> &[AuthorizationProof] {
        &self.authorizations
    }

    pub fn domain_payload(&self) -> &[u8] {
        &self.domain_payload
    }

    pub fn access_hints(&self) -> Option<&[u8]> {
        self.access_hints.as_deref()
    }

    fn encode_common_prefix(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&ENVELOPE_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&self.chain_id.to_le_bytes());
        bytes.push(self.execution_domain as u8);
        bytes.extend_from_slice(&self.valid_after_height.to_le_bytes());
        bytes.extend_from_slice(&self.valid_until_height.to_le_bytes());
        bytes.extend_from_slice(&self.principal.to_bytes());
        match self.fee_payer {
            None => bytes.push(0),
            Some(address) => {
                bytes.push(1);
                bytes.extend_from_slice(&address.to_bytes());
            }
        }
        bytes.extend_from_slice(&self.fee_caps.max_fee_per_weight.to_le_bytes());
        bytes.extend_from_slice(&self.fee_caps.max_priority_fee_per_weight.to_le_bytes());
        bytes.extend_from_slice(&self.fee_caps.max_weight.to_le_bytes());
    }

    fn encode_payload_and_hints(&self, bytes: &mut Vec<u8>) {
        write_varint(self.domain_payload.len() as u64, bytes);
        bytes.extend_from_slice(&self.domain_payload);
        match &self.access_hints {
            None => bytes.push(0),
            Some(hints) => {
                bytes.push(1);
                write_varint(hints.len() as u64, bytes);
                bytes.extend_from_slice(hints);
            }
        }
    }
}

fn validate_authorizations(
    execution_domain: ExecutionDomain,
    valid_after_height: u64,
    valid_until_height: u64,
    has_distinct_fee_payer: bool,
    authorizations: &[AuthorizationProof],
) -> Result<(), ExecutionEnvelopeError> {
    if authorizations.is_empty() || authorizations.len() > MAX_AUTH_PROOFS {
        return Err(ExecutionEnvelopeError::InvalidAuthorizationCount);
    }

    if authorizations[0].scope != AuthorizationScope::Principal {
        return Err(ExecutionEnvelopeError::MissingPrincipalAuthorization);
    }

    if authorizations.len() == 2 && authorizations[1].scope != AuthorizationScope::FeePayer {
        return Err(ExecutionEnvelopeError::DuplicateAuthorizationScope);
    }

    let has_fee_payer_authorization = authorizations
        .iter()
        .any(|authorization| authorization.scope == AuthorizationScope::FeePayer);

    if has_distinct_fee_payer && !has_fee_payer_authorization {
        return Err(ExecutionEnvelopeError::MissingFeePayerAuthorization);
    }
    if !has_distinct_fee_payer && has_fee_payer_authorization {
        return Err(ExecutionEnvelopeError::UnexpectedFeePayerAuthorization);
    }
    if has_distinct_fee_payer && authorizations.len() != 2 {
        return Err(ExecutionEnvelopeError::MissingFeePayerAuthorization);
    }
    if !has_distinct_fee_payer && authorizations.len() != 1 {
        return Err(ExecutionEnvelopeError::UnexpectedFeePayerAuthorization);
    }

    let uses_ethereum_authorization = authorizations
        .iter()
        .any(|authorization| authorization.scheme == AuthorizationScheme::EthereumEcdsaV1);
    if uses_ethereum_authorization {
        if execution_domain != ExecutionDomain::Evm {
            return Err(ExecutionEnvelopeError::EthereumAuthorizationOutsideEvm);
        }
        if valid_after_height != 0 || valid_until_height != u64::MAX {
            return Err(ExecutionEnvelopeError::EthereumAuthorizationRequiresNeutralHeightWindow);
        }
    }

    Ok(())
}

fn read_u8(decoder: &mut Decoder<'_>) -> Result<u8, PrimitiveError> {
    Ok(decoder.read_bytes(1)?[0])
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExecutionEnvelopeError {
    #[error(transparent)]
    Primitive(#[from] PrimitiveError),
    #[error(transparent)]
    ExecutionAddress(#[from] ExecutionAddressError),
    #[error("unsupported execution envelope version {0}")]
    UnsupportedVersion(u16),
    #[error("unknown execution domain {0:#04x}")]
    UnknownDomain(u8),
    #[error("unknown authorization scope {0:#04x}")]
    UnknownAuthorizationScope(u8),
    #[error("unknown authorization scheme {0:#06x}")]
    UnknownAuthorizationScheme(u16),
    #[error("invalid canonical option flag {0}")]
    InvalidOptionFlag(u8),
    #[error("present fee payer must differ from principal")]
    FeePayerEqualsPrincipal,
    #[error("valid_after_height exceeds valid_until_height")]
    InvalidHeightWindow,
    #[error("max_weight must be nonzero")]
    ZeroMaxWeight,
    #[error("max_priority_fee_per_weight exceeds max_fee_per_weight")]
    PriorityFeeExceedsMaxFee,
    #[error("authorization proof has invalid outer length for its scheme")]
    InvalidAuthorizationProofLength,
    #[error("authorization count must be exactly one or two")]
    InvalidAuthorizationCount,
    #[error("authorization scopes are duplicated or out of canonical order")]
    DuplicateAuthorizationScope,
    #[error("principal authorization is missing")]
    MissingPrincipalAuthorization,
    #[error("fee-payer authorization is present without a distinct fee payer")]
    UnexpectedFeePayerAuthorization,
    #[error("distinct fee payer requires exactly one fee-payer authorization")]
    MissingFeePayerAuthorization,
    #[error("Ethereum ECDSA source authorization is valid only in the EVM domain")]
    EthereumAuthorizationOutsideEvm,
    #[error("Ethereum source authorization requires neutral Oregon height validity")]
    EthereumAuthorizationRequiresNeutralHeightWindow,
    #[error("present access hints must be non-empty")]
    EmptyAccessHints,
    #[error("access hints exceed the V1 structural byte limit")]
    AccessHintsTooLarge,
    #[error("domain payload exceeds the V1 structural byte limit")]
    DomainPayloadTooLarge,
    #[error("execution envelope exceeds the V1 structural byte limit")]
    EnvelopeTooLarge,
}
