use oregon_contract_state::StateSource;
use oregon_primitives::execution_address::ExecutionAddress;

use crate::ExecutionJournalV1;

use super::accounting::{read_balance, write_balance};
use super::types::CoordinatorError;

pub(super) fn reserve_execution_funded_escrow<S: StateSource + ?Sized>(
    journal: &mut ExecutionJournalV1<'_, S>,
    payer: ExecutionAddress,
    max_escrow: u64,
) -> Result<(), CoordinatorError> {
    let current = read_balance(journal, payer)?;
    let available = current
        .checked_sub(max_escrow)
        .ok_or(CoordinatorError::InsufficientExecutionFunding)?;
    write_balance(journal, payer, available)
}
