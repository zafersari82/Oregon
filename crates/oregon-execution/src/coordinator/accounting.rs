use oregon_contract_state::{
    StateSource, balance_key, decode_accounting_u64, encode_accounting_u64,
    total_execution_balance_key,
};
use oregon_primitives::execution_address::ExecutionAddress;
use oregon_primitives::state_commitment::CommitmentDomainId;

use crate::ExecutionJournalV1;

use super::types::CoordinatorError;

const ACCOUNTING_DOMAIN: CommitmentDomainId = CommitmentDomainId::ExecutionAccounting;

pub(super) fn read_balance<S: StateSource + ?Sized>(
    journal: &ExecutionJournalV1<'_, S>,
    address: ExecutionAddress,
) -> Result<u64, CoordinatorError> {
    let value = journal
        .read(ACCOUNTING_DOMAIN, &balance_key(&address))
        .map_err(|_| CoordinatorError::AccountingInvariant)?;
    match value {
        Some(bytes) => {
            decode_accounting_u64(&bytes).map_err(|_| CoordinatorError::AccountingInvariant)
        }
        None => Ok(0),
    }
}

pub(super) fn write_balance<S: StateSource + ?Sized>(
    journal: &mut ExecutionJournalV1<'_, S>,
    address: ExecutionAddress,
    value: u64,
) -> Result<(), CoordinatorError> {
    let key = balance_key(&address);
    if value == 0 {
        journal
            .delete(ACCOUNTING_DOMAIN, &key)
            .map_err(|_| CoordinatorError::AccountingInvariant)
    } else {
        journal
            .put(ACCOUNTING_DOMAIN, &key, &encode_accounting_u64(value))
            .map_err(|_| CoordinatorError::AccountingInvariant)
    }
}

pub(super) fn read_total_execution_balance<S: StateSource + ?Sized>(
    journal: &ExecutionJournalV1<'_, S>,
) -> Result<u64, CoordinatorError> {
    let bytes = journal
        .read(ACCOUNTING_DOMAIN, total_execution_balance_key())
        .map_err(|_| CoordinatorError::AccountingInvariant)?
        .ok_or(CoordinatorError::AccountingInvariant)?;
    decode_accounting_u64(&bytes).map_err(|_| CoordinatorError::AccountingInvariant)
}
