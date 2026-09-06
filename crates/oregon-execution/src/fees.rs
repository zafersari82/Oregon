use std::collections::{BTreeMap, BTreeSet};

use oregon_primitives::fee_settlement::{
    ExecutionOutcome, FeeSettlementError, FeeSettlementReceiptV1, FeeSettlementReceiptV1Parts,
    FeeSourceKind,
};
use oregon_primitives::{ExecutionAddress, Hash256, MAX_SUPPLY_BASE_UNITS, domain_hash};
use thiserror::Error;

const CAPABILITY_VERSION_V1: u16 = 1;
const ESCROW_VERSION_V1: u16 = 1;
const CAPABILITY_DOMAIN: &[u8] = b"OREGON/FEE/CAPABILITY/V1\0";
const SOURCE_DOMAIN: &[u8] = b"OREGON/FEE/SOURCE/V1\0";
const ESCROW_DOMAIN: &[u8] = b"OREGON/FEE/ESCROW/V1\0";
const MAX_ESCROW_HISTORY_ENTRIES: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FundingCapabilityV1 {
    source_kind: FeeSourceKind,
    payer: ExecutionAddress,
    source_commitment: Hash256,
    available_amount: u64,
    source_sequence: u64,
    authorization_commitment: Hash256,
}

impl FundingCapabilityV1 {
    pub fn new(
        source_kind: FeeSourceKind,
        payer: ExecutionAddress,
        source_commitment: Hash256,
        available_amount: u64,
        source_sequence: u64,
        authorization_commitment: Hash256,
    ) -> Result<Self, FeeError> {
        if available_amount > MAX_SUPPLY_BASE_UNITS {
            return Err(FeeError::AmountExceedsMaximumSupply);
        }
        Ok(Self {
            source_kind,
            payer,
            source_commitment,
            available_amount,
            source_sequence,
            authorization_commitment,
        })
    }

    pub const fn source_kind(self) -> FeeSourceKind {
        self.source_kind
    }

    pub const fn payer(self) -> ExecutionAddress {
        self.payer
    }

    pub const fn source_commitment(self) -> Hash256 {
        self.source_commitment
    }

    pub const fn available_amount(self) -> u64 {
        self.available_amount
    }

    pub const fn source_sequence(self) -> u64 {
        self.source_sequence
    }

    pub const fn authorization_commitment(self) -> Hash256 {
        self.authorization_commitment
    }

    pub fn capability_id(self) -> Hash256 {
        let mut bytes = Vec::with_capacity(116);
        bytes.extend_from_slice(&CAPABILITY_VERSION_V1.to_le_bytes());
        bytes.push(self.source_kind as u8);
        bytes.extend_from_slice(&self.payer.to_bytes());
        bytes.extend_from_slice(self.source_commitment.as_bytes());
        bytes.extend_from_slice(&self.available_amount.to_le_bytes());
        bytes.extend_from_slice(&self.source_sequence.to_le_bytes());
        bytes.extend_from_slice(self.authorization_commitment.as_bytes());
        debug_assert_eq!(bytes.len(), 116);
        domain_hash(CAPABILITY_DOMAIN, &bytes)
    }

    fn source_id(self) -> Hash256 {
        let mut bytes = Vec::with_capacity(76);
        bytes.extend_from_slice(&CAPABILITY_VERSION_V1.to_le_bytes());
        bytes.push(self.source_kind as u8);
        bytes.extend_from_slice(&self.payer.to_bytes());
        bytes.extend_from_slice(self.source_commitment.as_bytes());
        bytes.extend_from_slice(&self.source_sequence.to_le_bytes());
        debug_assert_eq!(bytes.len(), 76);
        domain_hash(SOURCE_DOMAIN, &bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeTermsV1 {
    base_fee_per_weight: u64,
    max_fee_per_weight: u64,
    max_priority_fee_per_weight: u64,
    max_weight: u64,
    max_escrow: u64,
}

impl FeeTermsV1 {
    pub fn new(
        base_fee_per_weight: u64,
        max_fee_per_weight: u64,
        max_priority_fee_per_weight: u64,
        max_weight: u64,
    ) -> Result<Self, FeeError> {
        if max_weight == 0 {
            return Err(FeeError::ZeroMaxWeight);
        }
        if max_fee_per_weight < base_fee_per_weight {
            return Err(FeeError::MaxFeeBelowBaseFee);
        }
        if max_priority_fee_per_weight > max_fee_per_weight {
            return Err(FeeError::PriorityFeeExceedsMaxFee);
        }

        let max_escrow_wide = u128::from(max_weight) * u128::from(max_fee_per_weight);
        let max_escrow = u64::try_from(max_escrow_wide).map_err(|_| FeeError::ArithmeticOverflow)?;
        if max_escrow > MAX_SUPPLY_BASE_UNITS {
            return Err(FeeError::AmountExceedsMaximumSupply);
        }

        Ok(Self {
            base_fee_per_weight,
            max_fee_per_weight,
            max_priority_fee_per_weight,
            max_weight,
            max_escrow,
        })
    }

    pub const fn base_fee_per_weight(self) -> u64 {
        self.base_fee_per_weight
    }

    pub const fn max_fee_per_weight(self) -> u64 {
        self.max_fee_per_weight
    }

    pub const fn max_priority_fee_per_weight(self) -> u64 {
        self.max_priority_fee_per_weight
    }

    pub const fn max_weight(self) -> u64 {
        self.max_weight
    }

    pub const fn max_escrow(self) -> u64 {
        self.max_escrow
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EscrowTicketV1 {
    txid: Hash256,
    payer: ExecutionAddress,
    source_kind: FeeSourceKind,
    capability_id: Hash256,
    terms: FeeTermsV1,
}

impl EscrowTicketV1 {
    fn new(txid: Hash256, capability: FundingCapabilityV1, terms: FeeTermsV1) -> Self {
        Self {
            txid,
            payer: capability.payer,
            source_kind: capability.source_kind,
            capability_id: capability.capability_id(),
            terms,
        }
    }

    pub const fn txid(self) -> Hash256 {
        self.txid
    }

    pub const fn payer(self) -> ExecutionAddress {
        self.payer
    }

    pub const fn source_kind(self) -> FeeSourceKind {
        self.source_kind
    }

    pub const fn capability_id(self) -> Hash256 {
        self.capability_id
    }

    pub const fn terms(self) -> FeeTermsV1 {
        self.terms
    }

    pub fn escrow_id(self) -> Hash256 {
        let mut bytes = Vec::with_capacity(138);
        bytes.extend_from_slice(&ESCROW_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(self.txid.as_bytes());
        bytes.extend_from_slice(&self.payer.to_bytes());
        bytes.push(self.source_kind as u8);
        bytes.extend_from_slice(self.capability_id.as_bytes());
        for value in [
            self.terms.base_fee_per_weight,
            self.terms.max_fee_per_weight,
            self.terms.max_priority_fee_per_weight,
            self.terms.max_weight,
            self.terms.max_escrow,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        debug_assert_eq!(bytes.len(), 140);
        domain_hash(ESCROW_DOMAIN, &bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettlementResultV1 {
    receipt: FeeSettlementReceiptV1,
}

impl SettlementResultV1 {
    pub const fn receipt(&self) -> &FeeSettlementReceiptV1 {
        &self.receipt
    }

    pub const fn charged(self) -> u64 {
        self.receipt.charged()
    }

    pub const fn refund(self) -> u64 {
        self.receipt.refund()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenEscrow {
    ticket: EscrowTicketV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscrowBookV1 {
    max_open: usize,
    open: BTreeMap<Hash256, OpenEscrow>,
    consumed_capabilities: BTreeSet<Hash256>,
    consumed_sources: BTreeSet<Hash256>,
    settled: BTreeSet<Hash256>,
}

impl EscrowBookV1 {
    pub fn new(max_open: usize) -> Result<Self, FeeError> {
        if max_open == 0 {
            return Err(FeeError::ZeroEscrowCapacity);
        }
        Ok(Self {
            max_open,
            open: BTreeMap::new(),
            consumed_capabilities: BTreeSet::new(),
            consumed_sources: BTreeSet::new(),
            settled: BTreeSet::new(),
        })
    }

    pub fn open(
        &mut self,
        txid: Hash256,
        capability: FundingCapabilityV1,
        terms: FeeTermsV1,
    ) -> Result<EscrowTicketV1, FeeError> {
        let capability_id = capability.capability_id();
        if self.consumed_capabilities.contains(&capability_id) {
            return Err(FeeError::CapabilityAlreadyConsumed);
        }

        let source_id = capability.source_id();
        if self.consumed_sources.contains(&source_id) {
            return Err(FeeError::StaleFundingSource);
        }
        if self.open.len() >= self.max_open {
            return Err(FeeError::EscrowCapacityExceeded);
        }
        if self.consumed_capabilities.len() >= MAX_ESCROW_HISTORY_ENTRIES
            || self.consumed_sources.len() >= MAX_ESCROW_HISTORY_ENTRIES
        {
            return Err(FeeError::EscrowHistoryCapacityExceeded);
        }
        if capability.available_amount < terms.max_escrow {
            return Err(FeeError::InsufficientFunding);
        }

        let ticket = EscrowTicketV1::new(txid, capability, terms);
        let escrow_id = ticket.escrow_id();
        if self.open.contains_key(&escrow_id) || self.settled.contains(&escrow_id) {
            return Err(FeeError::EscrowAlreadyExists);
        }

        self.consumed_capabilities.insert(capability_id);
        self.consumed_sources.insert(source_id);
        self.open.insert(escrow_id, OpenEscrow { ticket });
        Ok(ticket)
    }

    pub fn settle(
        &mut self,
        escrow_id: Hash256,
        outcome: ExecutionOutcome,
        actual_weight: u64,
    ) -> Result<SettlementResultV1, FeeError> {
        if self.settled.contains(&escrow_id) {
            return Err(FeeError::EscrowAlreadySettled);
        }

        let open = self
            .open
            .get(&escrow_id)
            .copied()
            .ok_or(FeeError::UnknownEscrow)?;
        let ticket = open.ticket;
        let terms = ticket.terms;

        if actual_weight == 0 || actual_weight > terms.max_weight {
            return Err(FeeError::ActualWeightOutOfRange);
        }
        if self.settled.len() >= MAX_ESCROW_HISTORY_ENTRIES {
            return Err(FeeError::EscrowHistoryCapacityExceeded);
        }

        let offered_price =
            u128::from(terms.base_fee_per_weight) + u128::from(terms.max_priority_fee_per_weight);
        let effective_price_wide = u128::from(terms.max_fee_per_weight).min(offered_price);
        let actual_weight_wide = u128::from(actual_weight);
        let charged_wide = actual_weight_wide * effective_price_wide;
        let base_component_wide = actual_weight_wide * u128::from(terms.base_fee_per_weight);
        let priority_component_wide = charged_wide
            .checked_sub(base_component_wide)
            .ok_or(FeeError::ArithmeticOverflow)?;
        let refund_wide = u128::from(terms.max_escrow)
            .checked_sub(charged_wide)
            .ok_or(FeeError::ArithmeticOverflow)?;

        let effective_price =
            u64::try_from(effective_price_wide).map_err(|_| FeeError::ArithmeticOverflow)?;
        let charged = u64::try_from(charged_wide).map_err(|_| FeeError::ArithmeticOverflow)?;
        let base_component =
            u64::try_from(base_component_wide).map_err(|_| FeeError::ArithmeticOverflow)?;
        let priority_component =
            u64::try_from(priority_component_wide).map_err(|_| FeeError::ArithmeticOverflow)?;
        let refund = u64::try_from(refund_wide).map_err(|_| FeeError::ArithmeticOverflow)?;

        let receipt = FeeSettlementReceiptV1::new(FeeSettlementReceiptV1Parts {
            txid: ticket.txid,
            escrow_id,
            payer: ticket.payer,
            source_kind: ticket.source_kind,
            outcome,
            base_fee_per_weight: terms.base_fee_per_weight,
            max_fee_per_weight: terms.max_fee_per_weight,
            max_priority_fee_per_weight: terms.max_priority_fee_per_weight,
            max_weight: terms.max_weight,
            actual_weight,
            effective_price,
            base_component,
            priority_component,
            charged,
            refund,
        })?;

        self.open.remove(&escrow_id);
        self.settled.insert(escrow_id);
        Ok(SettlementResultV1 { receipt })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FeeError {
    #[error("max_weight must be nonzero")]
    ZeroMaxWeight,
    #[error("max fee per weight is below the block base fee")]
    MaxFeeBelowBaseFee,
    #[error("max priority fee per weight exceeds max fee per weight")]
    PriorityFeeExceedsMaxFee,
    #[error("fee arithmetic overflow")]
    ArithmeticOverflow,
    #[error("fee amount exceeds Oregon maximum supply")]
    AmountExceedsMaximumSupply,
    #[error("escrow capacity must be nonzero")]
    ZeroEscrowCapacity,
    #[error("open escrow capacity exceeded")]
    EscrowCapacityExceeded,
    #[error("fee funding is below maximum escrow")]
    InsufficientFunding,
    #[error("funding capability was already consumed")]
    CapabilityAlreadyConsumed,
    #[error("funding source state or sequence is stale")]
    StaleFundingSource,
    #[error("escrow id already exists")]
    EscrowAlreadyExists,
    #[error("escrow id is unknown")]
    UnknownEscrow,
    #[error("escrow was already settled")]
    EscrowAlreadySettled,
    #[error("actual weight is outside 1..=max_weight")]
    ActualWeightOutOfRange,
    #[error("escrow history capacity exceeded")]
    EscrowHistoryCapacityExceeded,
    #[error(transparent)]
    Receipt(#[from] FeeSettlementError),
}
