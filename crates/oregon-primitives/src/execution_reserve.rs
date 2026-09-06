//! Inactive Stage 3B protocol-reserve identifiers.
//!
//! The reserved locking program and domain-separated identifiers do not make
//! reserve outputs spendable and do not activate reserve handling in UTXO code.

use crate::{Hash256, domain_hash};

pub const EXECUTION_RESERVE_LOCKING_PROGRAM_V1: &[u8; 23] = b"OREGON/EXEC/RESERVE/V1\0";

pub fn reserve_transition_id(canonical_transition_bytes: &[u8]) -> Hash256 {
    domain_hash(
        b"OREGON/RESERVE/TRANSITION/V1\0",
        canonical_transition_bytes,
    )
}

pub fn reserve_outpoint_txid(transition_id: Hash256) -> Hash256 {
    domain_hash(
        b"OREGON/RESERVE/OUTPOINT/V1\0",
        transition_id.as_bytes(),
    )
}
