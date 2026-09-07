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
    if input.previous == Some(0) {
        return Err(ArithmeticError::InvalidPreviousSnapshot);
    }

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

    if input
        .previous
        .is_some_and(|amount| amount > MAX_SUPPLY_BASE_UNITS)
        || result > MAX_SUPPLY_BASE_UNITS
    {
        return Err(ArithmeticError::AmountOutOfRange);
    }

    Ok(result)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramClass {
    Reserve,
    Ordinary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelEntry {
    pub amount: u64,
    pub creation_height: u64,
    pub is_coinbase: bool,
    pub program: ProgramClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelSlot {
    pub key: u8,
    pub entry: ModelEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelState {
    pub slots: [Option<ModelSlot>; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateSnapshot {
    pub key: u8,
    pub amount: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateTransition {
    pub previous: Option<StateSnapshot>,
    pub new_key: Option<u8>,
    pub new_amount: u64,
    pub height: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelUndo {
    pub previous: Option<ModelSlot>,
    pub created: Option<ModelSlot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateError {
    PreviousReserveMismatch,
    MultipleLiveReserves,
    OutputCollision,
    UndoMismatch,
}

pub fn apply_state(state: &mut ModelState, transition: StateTransition) -> Result<(), StateError> {
    apply_state_with_undo(state, transition).map(|_| ())
}

pub fn apply_state_with_undo(
    state: &mut ModelState,
    transition: StateTransition,
) -> Result<ModelUndo, StateError> {
    let reserve_indices: [Option<usize>; 4] = core::array::from_fn(|index| {
        state.slots[index]
            .filter(|slot| slot.entry.program == ProgramClass::Reserve)
            .map(|_| index)
    });

    // Deliberately weakened for semantic RED evidence: two live reserves are
    // allowed to continue into ordinary previous-snapshot matching.

    let previous_index = match transition.previous {
        None => {
            if reserve_indices.iter().flatten().next().is_some() {
                return Err(StateError::PreviousReserveMismatch);
            }
            None
        }
        Some(expected) => {
            let matching = reserve_indices.iter().flatten().copied().find(|index| {
                state.slots[*index].is_some_and(|slot| {
                    slot.key == expected.key && slot.entry.amount == expected.amount
                })
            });
            matching
                .ok_or(StateError::PreviousReserveMismatch)
                .map(Some)?
        }
    };

    if let Some(new_key) = transition.new_key {
        if state.slots.iter().flatten().any(|slot| slot.key == new_key) {
            return Err(StateError::OutputCollision);
        }
    }

    let previous = previous_index.and_then(|index| state.slots[index]);
    let created = if transition.new_amount == 0 {
        None
    } else {
        let new_key = transition
            .new_key
            .ok_or(StateError::PreviousReserveMismatch)?;
        Some(ModelSlot {
            key: new_key,
            entry: ModelEntry {
                amount: transition.new_amount,
                creation_height: transition.height,
                is_coinbase: false,
                program: ProgramClass::Reserve,
            },
        })
    };

    let mut overlay = *state;
    if let Some(index) = previous_index {
        overlay.slots[index] = None;
    }

    if let Some(created) = created {
        let Some(index) = overlay.slots.iter().position(Option::is_none) else {
            return Err(StateError::OutputCollision);
        };
        overlay.slots[index] = Some(created);
    }

    *state = overlay;
    Ok(ModelUndo { previous, created })
}

pub fn undo_state(state: &mut ModelState, undo: &ModelUndo) -> Result<(), StateError> {
    let created_index = match undo.created {
        Some(expected) => {
            let live: [Option<usize>; 4] = core::array::from_fn(|index| {
                state.slots[index]
                    .filter(|slot| slot.entry.program == ProgramClass::Reserve)
                    .map(|_| index)
            });
            let mut live_indices = live.iter().flatten().copied();
            let Some(index) = live_indices.next() else {
                return Err(StateError::UndoMismatch);
            };
            if live_indices.next().is_some() {
                return Err(StateError::UndoMismatch);
            }
            let Some(actual) = state.slots[index] else {
                return Err(StateError::UndoMismatch);
            };
            if actual != expected {
                return Err(StateError::UndoMismatch);
            }
            Some(index)
        }
        None => {
            if state
                .slots
                .iter()
                .flatten()
                .any(|slot| slot.entry.program == ProgramClass::Reserve)
            {
                return Err(StateError::UndoMismatch);
            }
            None
        }
    };

    if let Some(previous) = undo.previous {
        if state
            .slots
            .iter()
            .flatten()
            .any(|slot| slot.key == previous.key)
        {
            return Err(StateError::UndoMismatch);
        }
    }

    let mut overlay = *state;
    if let Some(index) = created_index {
        overlay.slots[index] = None;
    }
    if let Some(previous) = undo.previous {
        let Some(index) = overlay.slots.iter().position(Option::is_none) else {
            return Err(StateError::UndoMismatch);
        };
        overlay.slots[index] = Some(previous);
    }

    *state = overlay;
    Ok(())
}
