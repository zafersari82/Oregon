pub const MAX_SUPPLY_BASE_UNITS: u64 = 100_000_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArithmeticInput {
    pub previous: Option<u64>,
    pub native_deposit_total: u64,
    pub execution_withdrawal_total: u64,
    pub execution_fee_total: u64,
    pub new_execution_balance_total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithmeticError {
    ArithmeticOverflow,
    ArithmeticUnderflow,
    ExecutionBalanceMismatch,
    InvalidPreviousSnapshot,
    AmountOutOfRange,
}

pub fn construct_arithmetic(input: ArithmeticInput) -> Result<u64, ArithmeticError> {
    // Deliberately weakened RED model: the production constructor rejects a present
    // zero snapshot before arithmetic. Omitting that guard must be caught by the
    // correspondence test before the real bounded model is implemented.
    let previous = input.previous.unwrap_or(0);
    let after_deposit = previous
        .checked_add(input.native_deposit_total)
        .ok_or(ArithmeticError::ArithmeticOverflow)?;
    let after_withdrawal = after_deposit
        .checked_sub(input.execution_withdrawal_total)
        .ok_or(ArithmeticError::ArithmeticUnderflow)?;
    let result = after_withdrawal
        .checked_sub(input.execution_fee_total)
        .ok_or(ArithmeticError::ArithmeticUnderflow)?;

    if result != input.new_execution_balance_total {
        return Err(ArithmeticError::ExecutionBalanceMismatch);
    }

    if input.previous.is_some_and(|amount| amount > MAX_SUPPLY_BASE_UNITS)
        || result > MAX_SUPPLY_BASE_UNITS
    {
        return Err(ArithmeticError::AmountOutOfRange);
    }

    Ok(result)
}
