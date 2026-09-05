use oregon_primitives::state_commitment::CommitmentDomainId;
use oregon_primitives::{Hash256, domain_hash};

use crate::StateError;

pub const SMT_DEPTH: usize = 256;
pub const MAX_STATE_KEY_BYTES: usize = 1_024;
pub const MAX_STATE_VALUE_BYTES: usize = 1_048_576;

const KEY_DOMAIN: &[u8] = b"OREGON/STATE/SMT/KEY/V1\0";
const VALUE_DOMAIN: &[u8] = b"OREGON/STATE/SMT/VALUE/V1\0";
const EMPTY_DOMAIN: &[u8] = b"OREGON/STATE/SMT/EMPTY/V1\0";
const LEAF_DOMAIN: &[u8] = b"OREGON/STATE/SMT/LEAF/V1\0";
const NODE_DOMAIN: &[u8] = b"OREGON/STATE/SMT/NODE/V1\0";

fn domain_prefix(domain: CommitmentDomainId) -> [u8; 2] {
    u16::from(domain).to_le_bytes()
}

pub fn path_key(domain: CommitmentDomainId, key: &[u8]) -> Result<Hash256, StateError> {
    if key.len() > MAX_STATE_KEY_BYTES {
        return Err(StateError::KeyTooLarge(key.len()));
    }
    let mut payload = Vec::with_capacity(2 + key.len());
    payload.extend_from_slice(&domain_prefix(domain));
    payload.extend_from_slice(key);
    Ok(domain_hash(KEY_DOMAIN, &payload))
}

pub fn value_hash(domain: CommitmentDomainId, value: &[u8]) -> Result<Hash256, StateError> {
    if value.len() > MAX_STATE_VALUE_BYTES {
        return Err(StateError::ValueTooLarge(value.len()));
    }
    let mut payload = Vec::with_capacity(2 + value.len());
    payload.extend_from_slice(&domain_prefix(domain));
    payload.extend_from_slice(value);
    Ok(domain_hash(VALUE_DOMAIN, &payload))
}

pub fn empty_hashes(domain: CommitmentDomainId) -> [Hash256; SMT_DEPTH + 1] {
    let prefix = domain_prefix(domain);
    let mut hashes = [Hash256::from_bytes([0u8; 32]); SMT_DEPTH + 1];
    hashes[SMT_DEPTH] = domain_hash(EMPTY_DOMAIN, &prefix);

    for depth in (0..SMT_DEPTH).rev() {
        hashes[depth] =
            branch_hash_unchecked(domain, depth as u16, hashes[depth + 1], hashes[depth + 1]);
    }
    hashes
}

pub fn leaf_hash(domain: CommitmentDomainId, path_key: Hash256, value_hash: Hash256) -> Hash256 {
    let mut payload = Vec::with_capacity(66);
    payload.extend_from_slice(&domain_prefix(domain));
    payload.extend_from_slice(path_key.as_bytes());
    payload.extend_from_slice(value_hash.as_bytes());
    domain_hash(LEAF_DOMAIN, &payload)
}

pub fn branch_hash(
    domain: CommitmentDomainId,
    depth: u16,
    left: Hash256,
    right: Hash256,
) -> Result<Hash256, StateError> {
    if depth as usize >= SMT_DEPTH {
        return Err(StateError::DepthOutOfRange(depth as usize));
    }
    Ok(branch_hash_unchecked(domain, depth, left, right))
}

fn branch_hash_unchecked(
    domain: CommitmentDomainId,
    depth: u16,
    left: Hash256,
    right: Hash256,
) -> Hash256 {
    let mut payload = Vec::with_capacity(68);
    payload.extend_from_slice(&domain_prefix(domain));
    payload.extend_from_slice(&depth.to_le_bytes());
    payload.extend_from_slice(left.as_bytes());
    payload.extend_from_slice(right.as_bytes());
    domain_hash(NODE_DOMAIN, &payload)
}

pub fn path_bit(path_key: Hash256, depth: usize) -> Result<bool, StateError> {
    if depth >= SMT_DEPTH {
        return Err(StateError::DepthOutOfRange(depth));
    }
    Ok(path_bit_unchecked(path_key, depth))
}

pub(crate) fn path_bit_unchecked(path_key: Hash256, depth: usize) -> bool {
    let byte = path_key.as_bytes()[depth / 8];
    (byte & (0x80 >> (depth % 8))) != 0
}
