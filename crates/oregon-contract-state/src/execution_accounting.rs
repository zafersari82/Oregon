use oregon_primitives::execution_address::ExecutionAddress;
use oregon_primitives::state_commitment::CommitmentDomainId;
use oregon_primitives::OutPoint;

use crate::{StateError, StateWrite, StateWriteSet};

const BALANCE_KEY_PREFIX_V1: &[u8] = b"acct/v1/balance/";
const SEQUENCE_KEY_PREFIX_V1: &[u8] = b"acct/v1/sequence/";
const TOTAL_EXECUTION_BALANCE_KEY_V1: &[u8] = b"acct/v1/total_execution_balance";
const RESERVE_OUTPOINT_KEY_V1: &[u8] = b"acct/v1/reserve_outpoint";

pub fn balance_key(address: &ExecutionAddress) -> Vec<u8> {
    let mut key = Vec::with_capacity(BALANCE_KEY_PREFIX_V1.len() + 33);
    key.extend_from_slice(BALANCE_KEY_PREFIX_V1);
    key.extend_from_slice(&address.to_bytes());
    key
}

pub fn sequence_key(address: &ExecutionAddress) -> Vec<u8> {
    let mut key = Vec::with_capacity(SEQUENCE_KEY_PREFIX_V1.len() + 33);
    key.extend_from_slice(SEQUENCE_KEY_PREFIX_V1);
    key.extend_from_slice(&address.to_bytes());
    key
}

pub const fn total_execution_balance_key() -> &'static [u8] {
    TOTAL_EXECUTION_BALANCE_KEY_V1
}

pub const fn reserve_outpoint_key() -> &'static [u8] {
    RESERVE_OUTPOINT_KEY_V1
}

pub fn encode_accounting_u64(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

pub fn encode_reserve_outpoint(outpoint: Option<OutPoint>) -> Vec<u8> {
    let Some(outpoint) = outpoint else {
        return vec![0x00];
    };

    let mut bytes = Vec::with_capacity(37);
    bytes.push(0x01);
    bytes.extend_from_slice(outpoint.txid.as_bytes());
    bytes.extend_from_slice(&outpoint.index.to_le_bytes());
    bytes
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionAccountingWritesV1 {
    pub balances: Vec<(ExecutionAddress, Option<u64>)>,
    pub sequences: Vec<(ExecutionAddress, u64)>,
    pub total_execution_balance: u64,
    pub reserve_outpoint: Option<OutPoint>,
}

impl ExecutionAccountingWritesV1 {
    pub fn write_set(&self) -> Result<StateWriteSet, StateError> {
        let mut writes = Vec::with_capacity(self.balances.len() + self.sequences.len() + 2);

        for (address, balance) in &self.balances {
            let key = balance_key(address);
            match balance {
                Some(value) if *value != 0 => {
                    writes.push(StateWrite::put(key, encode_accounting_u64(*value)));
                }
                Some(_) | None => writes.push(StateWrite::delete(key)),
            }
        }

        for (address, sequence) in &self.sequences {
            writes.push(StateWrite::put(
                sequence_key(address),
                encode_accounting_u64(*sequence),
            ));
        }

        writes.push(StateWrite::put(
            total_execution_balance_key().to_vec(),
            encode_accounting_u64(self.total_execution_balance),
        ));
        writes.push(StateWrite::put(
            reserve_outpoint_key().to_vec(),
            encode_reserve_outpoint(self.reserve_outpoint),
        ));

        StateWriteSet::new(CommitmentDomainId::ExecutionAccounting, writes)
    }
}
