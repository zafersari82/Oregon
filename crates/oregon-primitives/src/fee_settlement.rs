//! Inactive Stage 3B fee-settlement receipt primitives.
//!
//! These types freeze canonical settlement bytes and arithmetic checks. They do
//! not activate execution, debit balances, mutate UTXOs or change coinbase rules.

use thiserror::Error;

use crate::execution_address::{ExecutionAddress, ExecutionAddressError};
use crate::{Hash256, MAX_SUPPLY_BASE_UNITS, PrimitiveError, domain_hash};

const RECEIPT_VERSION: u16 = 1;
const RECEIPT_DOMAIN: &[u8] = b"OREGON/FEE/RECEIPT/V1\0";
const RECEIPT_BYTES: usize = 181;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FeeSourceKind {
    NativeUtxo = 0x01,
    ExecutionBalance = 0x02,
}

impl TryFrom<u8> for FeeSourceKind {
    type Error = FeeSettlementError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Self::NativeUtxo),
            0x02 => Ok(Self::ExecutionBalance),
            _ => Err(FeeSettlementError::UnknownFeeSourceKind(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExecutionOutcome {
    Committed = 0x00,
    Reverted = 0x01,
    ResourceExhausted = 0x02,
}

impl TryFrom<u8> for ExecutionOutcome {
    type Error = FeeSettlementError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Committed),
            0x01 => Ok(Self::Reverted),
            0x02 => Ok(Self::ResourceExhausted),
            _ => Err(FeeSettlementError::UnknownExecutionOutcome(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeSettlementReceiptV1Parts {
    pub txid: Hash256,
    pub escrow_id: Hash256,
    pub payer: ExecutionAddress,
    pub source_kind: FeeSourceKind,
    pub outcome: ExecutionOutcome,
    pub base_fee_per_weight: u64,
    pub max_fee_per_weight: u64,
    pub max_priority_fee_per_weight: u64,
    pub max_weight: u64,
    pub actual_weight: u64,
    pub effective_price: u64,
    pub base_component: u64,
    pub priority_component: u64,
    pub charged: u64,
    pub refund: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeSettlementReceiptV1 {
    parts: FeeSettlementReceiptV1Parts,
}

impl FeeSettlementReceiptV1 {
    pub fn new(parts: FeeSettlementReceiptV1Parts) -> Result<Self, FeeSettlementError> {
        validate_arithmetic(&parts)?;
        Ok(Self { parts })
    }

    pub const fn parts(&self) -> &FeeSettlementReceiptV1Parts {
        &self.parts
    }

    pub const fn txid(&self) -> Hash256 {
        self.parts.txid
    }

    pub const fn escrow_id(&self) -> Hash256 {
        self.parts.escrow_id
    }

    pub const fn payer(&self) -> ExecutionAddress {
        self.parts.payer
    }

    pub const fn source_kind(&self) -> FeeSourceKind {
        self.parts.source_kind
    }

    pub const fn outcome(&self) -> ExecutionOutcome {
        self.parts.outcome
    }

    pub const fn actual_weight(&self) -> u64 {
        self.parts.actual_weight
    }

    pub const fn charged(&self) -> u64 {
        self.parts.charged
    }

    pub const fn refund(&self) -> u64 {
        self.parts.refund
    }

    pub fn encode(&self) -> [u8; RECEIPT_BYTES] {
        let mut bytes = [0u8; RECEIPT_BYTES];
        let mut offset = 0usize;

        write(&mut bytes, &mut offset, &RECEIPT_VERSION.to_le_bytes());
        write(&mut bytes, &mut offset, self.parts.txid.as_bytes());
        write(&mut bytes, &mut offset, self.parts.escrow_id.as_bytes());
        write(&mut bytes, &mut offset, &self.parts.payer.to_bytes());
        write(&mut bytes, &mut offset, &[self.parts.source_kind as u8]);
        write(&mut bytes, &mut offset, &[self.parts.outcome as u8]);
        for value in [
            self.parts.base_fee_per_weight,
            self.parts.max_fee_per_weight,
            self.parts.max_priority_fee_per_weight,
            self.parts.max_weight,
            self.parts.actual_weight,
            self.parts.effective_price,
            self.parts.base_component,
            self.parts.priority_component,
            self.parts.charged,
            self.parts.refund,
        ] {
            write(&mut bytes, &mut offset, &value.to_le_bytes());
        }

        debug_assert_eq!(offset, RECEIPT_BYTES);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FeeSettlementError> {
        if bytes.len() != RECEIPT_BYTES {
            return Err(FeeSettlementError::InvalidReceiptLength(bytes.len()));
        }

        let version = u16::from_le_bytes([bytes[0], bytes[1]]);
        if version != RECEIPT_VERSION {
            return Err(FeeSettlementError::UnsupportedReceiptVersion(version));
        }

        let txid = Hash256::from_slice(&bytes[2..34])?;
        let escrow_id = Hash256::from_slice(&bytes[34..66])?;
        let payer = ExecutionAddress::from_slice(&bytes[66..99])?;
        let source_kind = FeeSourceKind::try_from(bytes[99])?;
        let outcome = ExecutionOutcome::try_from(bytes[100])?;

        let mut offset = 101usize;
        let base_fee_per_weight = read_u64(bytes, &mut offset);
        let max_fee_per_weight = read_u64(bytes, &mut offset);
        let max_priority_fee_per_weight = read_u64(bytes, &mut offset);
        let max_weight = read_u64(bytes, &mut offset);
        let actual_weight = read_u64(bytes, &mut offset);
        let effective_price = read_u64(bytes, &mut offset);
        let base_component = read_u64(bytes, &mut offset);
        let priority_component = read_u64(bytes, &mut offset);
        let charged = read_u64(bytes, &mut offset);
        let refund = read_u64(bytes, &mut offset);
        debug_assert_eq!(offset, RECEIPT_BYTES);

        Self::new(FeeSettlementReceiptV1Parts {
            txid,
            escrow_id,
            payer,
            source_kind,
            outcome,
            base_fee_per_weight,
            max_fee_per_weight,
            max_priority_fee_per_weight,
            max_weight,
            actual_weight,
            effective_price,
            base_component,
            priority_component,
            charged,
            refund,
        })
    }

    pub fn receipt_id(&self) -> Hash256 {
        domain_hash(RECEIPT_DOMAIN, &self.encode())
    }
}

fn validate_arithmetic(parts: &FeeSettlementReceiptV1Parts) -> Result<(), FeeSettlementError> {
    if parts.max_weight == 0 || parts.actual_weight == 0 || parts.actual_weight > parts.max_weight {
        return Err(FeeSettlementError::InvalidActualWeight);
    }
    if parts.max_fee_per_weight < parts.base_fee_per_weight {
        return Err(FeeSettlementError::MaxFeeBelowBaseFee);
    }
    if parts.max_priority_fee_per_weight > parts.max_fee_per_weight {
        return Err(FeeSettlementError::PriorityFeeExceedsMaxFee);
    }

    let max_escrow = u128::from(parts.max_weight) * u128::from(parts.max_fee_per_weight);
    if max_escrow > u128::from(u64::MAX) {
        return Err(FeeSettlementError::ArithmeticOverflow);
    }
    if max_escrow > u128::from(MAX_SUPPLY_BASE_UNITS) {
        return Err(FeeSettlementError::AmountExceedsMaximumSupply);
    }

    let offered_price = u128::from(parts.base_fee_per_weight)
        + u128::from(parts.max_priority_fee_per_weight);
    let effective_price = u128::from(parts.max_fee_per_weight).min(offered_price);
    let actual_weight = u128::from(parts.actual_weight);
    let charged = actual_weight * effective_price;
    let base_component = actual_weight * u128::from(parts.base_fee_per_weight);
    let priority_component = charged
        .checked_sub(base_component)
        .ok_or(FeeSettlementError::InconsistentArithmetic)?;
    let refund = max_escrow
        .checked_sub(charged)
        .ok_or(FeeSettlementError::InconsistentArithmetic)?;

    if effective_price != u128::from(parts.effective_price)
        || base_component != u128::from(parts.base_component)
        || priority_component != u128::from(parts.priority_component)
        || charged != u128::from(parts.charged)
        || refund != u128::from(parts.refund)
    {
        return Err(FeeSettlementError::InconsistentArithmetic);
    }

    Ok(())
}

fn write<const N: usize>(target: &mut [u8; N], offset: &mut usize, bytes: &[u8]) {
    let end = *offset + bytes.len();
    target[*offset..end].copy_from_slice(bytes);
    *offset = end;
}

fn read_u64(bytes: &[u8], offset: &mut usize) -> u64 {
    let end = *offset + 8;
    let value = u64::from_le_bytes(bytes[*offset..end].try_into().expect("fixed receipt width"));
    *offset = end;
    value
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FeeSettlementError {
    #[error("unknown fee source kind {0}")]
    UnknownFeeSourceKind(u8),
    #[error("unknown execution outcome {0}")]
    UnknownExecutionOutcome(u8),
    #[error("fee settlement receipt must be exactly 181 bytes, got {0}")]
    InvalidReceiptLength(usize),
    #[error("unsupported fee settlement receipt version {0}")]
    UnsupportedReceiptVersion(u16),
    #[error("actual weight must be within 1..=max_weight")]
    InvalidActualWeight,
    #[error("max fee per weight is below the block base fee")]
    MaxFeeBelowBaseFee,
    #[error("max priority fee per weight exceeds max fee per weight")]
    PriorityFeeExceedsMaxFee,
    #[error("fee arithmetic overflow")]
    ArithmeticOverflow,
    #[error("maximum fee escrow exceeds Oregon maximum supply")]
    AmountExceedsMaximumSupply,
    #[error("fee settlement arithmetic fields are inconsistent")]
    InconsistentArithmetic,
    #[error(transparent)]
    Primitive(#[from] PrimitiveError),
    #[error(transparent)]
    Address(#[from] ExecutionAddressError),
}
