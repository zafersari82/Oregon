use crate::execution_address::{ExecutionAddress, ExecutionAddressError};
use crate::execution_envelope::ExecutionDomain;
use crate::execution_event::MAX_EXECUTION_EVENTS_V1;
use crate::fee_settlement::{ExecutionOutcome, FeeSettlementReceiptV1};
use crate::{Hash256, PrimitiveError, domain_hash};
use thiserror::Error;

pub const EXECUTION_RECEIPT_BYTES_V1: usize = 259;
pub const MAX_EXECUTION_RETURN_DATA_BYTES_V1: usize = 262_144;

const EXECUTION_RECEIPT_VERSION_V1: u16 = 1;
const RETURN_DOMAIN: &[u8] = b"OREGON/EXEC/RETURN/V1\0";
const OUTBOX_EFFECT_DOMAIN: &[u8] = b"OREGON/EXEC/OUTBOX-EFFECT/V1\0";
const EXECUTION_RECEIPT_DOMAIN: &[u8] = b"OREGON/EXEC/RECEIPT/V1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExecutionReceiptOutcomeV1 {
    Committed = 0x00,
    Reverted = 0x01,
    Trapped = 0x02,
    ResourceExhausted = 0x03,
}

impl TryFrom<u8> for ExecutionReceiptOutcomeV1 {
    type Error = ExecutionReceiptError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Committed),
            0x01 => Ok(Self::Reverted),
            0x02 => Ok(Self::Trapped),
            0x03 => Ok(Self::ResourceExhausted),
            _ => Err(ExecutionReceiptError::UnknownOutcome(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionReceiptV1Parts {
    pub txid: Hash256,
    pub execution_domain: ExecutionDomain,
    pub outcome: ExecutionReceiptOutcomeV1,
    pub trap_code: u16,
    pub fee_payer: ExecutionAddress,
    pub actual_weight: u64,
    pub fee_charged: u64,
    pub fee_settlement_receipt_id: Hash256,
    pub state_effect_root: Hash256,
    pub events_root: Hash256,
    pub event_count: u32,
    pub return_data_hash: Hash256,
    pub return_data_len: u32,
    pub outbox_effect_root: Hash256,
    pub outbox_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionReceiptV1 {
    parts: ExecutionReceiptV1Parts,
}

impl ExecutionReceiptV1 {
    pub fn new(
        parts: ExecutionReceiptV1Parts,
        fee_receipt: &FeeSettlementReceiptV1,
    ) -> Result<Self, ExecutionReceiptError> {
        validate_structural(&parts)?;

        let expected_fee_outcome = match parts.outcome {
            ExecutionReceiptOutcomeV1::Committed => ExecutionOutcome::Committed,
            ExecutionReceiptOutcomeV1::Reverted | ExecutionReceiptOutcomeV1::Trapped => {
                ExecutionOutcome::Reverted
            }
            ExecutionReceiptOutcomeV1::ResourceExhausted => ExecutionOutcome::ResourceExhausted,
        };

        if parts.txid != fee_receipt.txid()
            || parts.fee_payer != fee_receipt.payer()
            || parts.actual_weight != fee_receipt.actual_weight()
            || parts.fee_charged != fee_receipt.charged()
            || parts.fee_settlement_receipt_id != fee_receipt.receipt_id()
            || fee_receipt.outcome() != expected_fee_outcome
        {
            return Err(ExecutionReceiptError::FeeReceiptMismatch);
        }

        Ok(Self { parts })
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ExecutionReceiptError> {
        if bytes.len() != EXECUTION_RECEIPT_BYTES_V1 {
            return Err(ExecutionReceiptError::InvalidReceiptLength(bytes.len()));
        }

        let version = u16::from_le_bytes([bytes[0], bytes[1]]);
        if version != EXECUTION_RECEIPT_VERSION_V1 {
            return Err(ExecutionReceiptError::UnsupportedVersion(version));
        }

        let txid = Hash256::from_slice(&bytes[2..34])?;
        let execution_domain = decode_execution_domain(bytes[34])?;
        let outcome = ExecutionReceiptOutcomeV1::try_from(bytes[35])?;
        let trap_code = u16::from_le_bytes([bytes[36], bytes[37]]);
        let fee_payer = ExecutionAddress::from_slice(&bytes[38..71])?;
        let actual_weight = read_u64(bytes, 71);
        let fee_charged = read_u64(bytes, 79);
        let fee_settlement_receipt_id = Hash256::from_slice(&bytes[87..119])?;
        let state_effect_root = Hash256::from_slice(&bytes[119..151])?;
        let events_root = Hash256::from_slice(&bytes[151..183])?;
        let event_count = read_u32(bytes, 183);
        let return_data_hash = Hash256::from_slice(&bytes[187..219])?;
        let return_data_len = read_u32(bytes, 219);
        let outbox_effect_root = Hash256::from_slice(&bytes[223..255])?;
        let outbox_count = read_u32(bytes, 255);

        let parts = ExecutionReceiptV1Parts {
            txid,
            execution_domain,
            outcome,
            trap_code,
            fee_payer,
            actual_weight,
            fee_charged,
            fee_settlement_receipt_id,
            state_effect_root,
            events_root,
            event_count,
            return_data_hash,
            return_data_len,
            outbox_effect_root,
            outbox_count,
        };
        validate_structural(&parts)?;
        Ok(Self { parts })
    }

    pub const fn parts(&self) -> &ExecutionReceiptV1Parts {
        &self.parts
    }

    pub fn encode(&self) -> [u8; EXECUTION_RECEIPT_BYTES_V1] {
        let mut bytes = [0u8; EXECUTION_RECEIPT_BYTES_V1];
        let mut offset = 0usize;

        write(&mut bytes, &mut offset, &EXECUTION_RECEIPT_VERSION_V1.to_le_bytes());
        write(&mut bytes, &mut offset, self.parts.txid.as_bytes());
        write(
            &mut bytes,
            &mut offset,
            &[self.parts.execution_domain as u8],
        );
        write(&mut bytes, &mut offset, &[self.parts.outcome as u8]);
        write(&mut bytes, &mut offset, &self.parts.trap_code.to_le_bytes());
        write(&mut bytes, &mut offset, &self.parts.fee_payer.to_bytes());
        write(&mut bytes, &mut offset, &self.parts.actual_weight.to_le_bytes());
        write(&mut bytes, &mut offset, &self.parts.fee_charged.to_le_bytes());
        write(
            &mut bytes,
            &mut offset,
            self.parts.fee_settlement_receipt_id.as_bytes(),
        );
        write(
            &mut bytes,
            &mut offset,
            self.parts.state_effect_root.as_bytes(),
        );
        write(&mut bytes, &mut offset, self.parts.events_root.as_bytes());
        write(&mut bytes, &mut offset, &self.parts.event_count.to_le_bytes());
        write(
            &mut bytes,
            &mut offset,
            self.parts.return_data_hash.as_bytes(),
        );
        write(
            &mut bytes,
            &mut offset,
            &self.parts.return_data_len.to_le_bytes(),
        );
        write(
            &mut bytes,
            &mut offset,
            self.parts.outbox_effect_root.as_bytes(),
        );
        write(&mut bytes, &mut offset, &self.parts.outbox_count.to_le_bytes());

        debug_assert_eq!(offset, EXECUTION_RECEIPT_BYTES_V1);
        bytes
    }

    pub fn receipt_id(&self) -> Hash256 {
        domain_hash(EXECUTION_RECEIPT_DOMAIN, &self.encode())
    }
}

pub fn return_data_hash(return_data: &[u8]) -> Hash256 {
    domain_hash(RETURN_DOMAIN, return_data)
}

pub fn empty_outbox_effect_root() -> Hash256 {
    domain_hash(OUTBOX_EFFECT_DOMAIN, &0u32.to_le_bytes())
}

fn validate_structural(parts: &ExecutionReceiptV1Parts) -> Result<(), ExecutionReceiptError> {
    let trapped = parts.outcome == ExecutionReceiptOutcomeV1::Trapped;
    if trapped != (parts.trap_code != 0) {
        return Err(ExecutionReceiptError::InvalidTrapCode);
    }

    let event_count = usize::try_from(parts.event_count)
        .map_err(|_| ExecutionReceiptError::TooManyEvents)?;
    if event_count > MAX_EXECUTION_EVENTS_V1 {
        return Err(ExecutionReceiptError::TooManyEvents);
    }

    let return_data_len = usize::try_from(parts.return_data_len)
        .map_err(|_| ExecutionReceiptError::ReturnDataTooLarge)?;
    if return_data_len > MAX_EXECUTION_RETURN_DATA_BYTES_V1 {
        return Err(ExecutionReceiptError::ReturnDataTooLarge);
    }

    Ok(())
}

fn decode_execution_domain(value: u8) -> Result<ExecutionDomain, ExecutionReceiptError> {
    match value {
        0x10 => Ok(ExecutionDomain::Native),
        0x11 => Ok(ExecutionDomain::Evm),
        0x12 => Ok(ExecutionDomain::Wasm),
        0x13 => Ok(ExecutionDomain::System),
        _ => Err(ExecutionReceiptError::UnknownExecutionDomain(value)),
    }
}

fn write<const N: usize>(target: &mut [u8; N], offset: &mut usize, bytes: &[u8]) {
    let end = *offset + bytes.len();
    target[*offset..end].copy_from_slice(bytes);
    *offset = end;
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("fixed receipt width"))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed receipt width"))
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExecutionReceiptError {
    #[error("execution event exceeds the four-topic V1 limit")]
    TooManyEventTopics,
    #[error("execution event data exceeds the 65536-byte V1 limit")]
    EventDataTooLarge,
    #[error("execution event count exceeds the 256-event V1 limit")]
    TooManyEvents,
    #[error("ExecutionReceipts cannot appear in Phase-A state effects")]
    ReceiptDomainInStateEffects,
    #[error("state-effect domain is outside the Stage 4B transaction proposal")]
    InvalidEffectDomain,
    #[error("state-effect commitment scheme is invalid for its domain")]
    InvalidEffectScheme,
    #[error("state-effect domains are duplicated")]
    DuplicateEffectDomain,
    #[error("state-effect descriptors are not in strictly increasing domain order")]
    NonCanonicalEffectOrder,
    #[error("state-effect descriptor count exceeds the V1 commitment ceiling")]
    TooManyStateEffects,
    #[error("execution receipt must be exactly 259 bytes, got {0}")]
    InvalidReceiptLength(usize),
    #[error("unsupported execution receipt version {0}")]
    UnsupportedVersion(u16),
    #[error("unknown execution receipt outcome {0}")]
    UnknownOutcome(u8),
    #[error("unknown execution receipt domain {0:#04x}")]
    UnknownExecutionDomain(u8),
    #[error("trap code must be nonzero exactly for a trapped execution receipt")]
    InvalidTrapCode,
    #[error("execution receipt disagrees with authoritative fee settlement receipt")]
    FeeReceiptMismatch,
    #[error("execution return data exceeds the 262144-byte V1 limit")]
    ReturnDataTooLarge,
    #[error(transparent)]
    Primitive(#[from] PrimitiveError),
    #[error(transparent)]
    Address(#[from] ExecutionAddressError),
}
