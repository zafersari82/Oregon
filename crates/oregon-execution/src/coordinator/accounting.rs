use oregon_contract_state::{
    StateSource, balance_key, decode_accounting_u64, encode_accounting_u64,
    total_execution_balance_key,
};
use oregon_primitives::execution_address::ExecutionAddress;
use oregon_primitives::state_commitment::CommitmentDomainId;

use crate::{ExecutionJournalV1, JournalError};

use super::types::CoordinatorError;

const ACCOUNTING_DOMAIN: CommitmentDomainId = CommitmentDomainId::ExecutionAccounting;

pub(super) trait CoordinatorJournalV1 {
    fn read(
        &self,
        domain: CommitmentDomainId,
        key: &[u8],
    ) -> Result<Option<Vec<u8>>, JournalError>;

    fn put(
        &mut self,
        domain: CommitmentDomainId,
        key: &[u8],
        value: &[u8],
    ) -> Result<(), JournalError>;

    fn delete(&mut self, domain: CommitmentDomainId, key: &[u8]) -> Result<(), JournalError>;

    fn begin_frame(&mut self) -> Result<(), JournalError>;

    fn commit_frame(&mut self) -> Result<(), JournalError>;

    fn revert_frame(&mut self) -> Result<(), JournalError>;
}

impl<S: StateSource + ?Sized> CoordinatorJournalV1 for ExecutionJournalV1<'_, S> {
    fn read(
        &self,
        domain: CommitmentDomainId,
        key: &[u8],
    ) -> Result<Option<Vec<u8>>, JournalError> {
        ExecutionJournalV1::read(self, domain, key)
    }

    fn put(
        &mut self,
        domain: CommitmentDomainId,
        key: &[u8],
        value: &[u8],
    ) -> Result<(), JournalError> {
        ExecutionJournalV1::put(self, domain, key, value)
    }

    fn delete(&mut self, domain: CommitmentDomainId, key: &[u8]) -> Result<(), JournalError> {
        ExecutionJournalV1::delete(self, domain, key)
    }

    fn begin_frame(&mut self) -> Result<(), JournalError> {
        ExecutionJournalV1::begin_frame(self)
    }

    fn commit_frame(&mut self) -> Result<(), JournalError> {
        ExecutionJournalV1::commit_frame(self)
    }

    fn revert_frame(&mut self) -> Result<(), JournalError> {
        ExecutionJournalV1::revert_frame(self)
    }
}

pub(super) fn read_balance(
    journal: &dyn CoordinatorJournalV1,
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

pub(super) fn write_balance(
    journal: &mut dyn CoordinatorJournalV1,
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

pub(super) fn read_total_execution_balance(
    journal: &dyn CoordinatorJournalV1,
) -> Result<u64, CoordinatorError> {
    let bytes = journal
        .read(ACCOUNTING_DOMAIN, total_execution_balance_key())
        .map_err(|_| CoordinatorError::AccountingInvariant)?
        .ok_or(CoordinatorError::AccountingInvariant)?;
    decode_accounting_u64(&bytes).map_err(|_| CoordinatorError::AccountingInvariant)
}

pub(super) fn write_total_execution_balance(
    journal: &mut dyn CoordinatorJournalV1,
    value: u64,
) -> Result<(), CoordinatorError> {
    journal
        .put(
            ACCOUNTING_DOMAIN,
            total_execution_balance_key(),
            &encode_accounting_u64(value),
        )
        .map_err(|_| CoordinatorError::AccountingInvariant)
}
