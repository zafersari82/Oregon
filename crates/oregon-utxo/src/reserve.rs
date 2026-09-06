use oregon_primitives::execution_reserve::{
    EXECUTION_RESERVE_LOCKING_PROGRAM_V1, reserve_outpoint_txid, reserve_transition_id,
};
use oregon_primitives::{Amount, Hash256, OutPoint, TxOutput};
use thiserror::Error;

use crate::{UtxoEntry, UtxoState};

const RESERVE_TRANSITION_VERSION_V1: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReservePoolSnapshotV1 {
    pub outpoint: OutPoint,
    pub amount: u64,
}

impl ReservePoolSnapshotV1 {
    #[cfg(test)]
    fn test(amount: u64) -> Self {
        Self {
            outpoint: OutPoint {
                txid: Hash256::from_bytes([0x90; 32]),
                index: 0,
            },
            amount,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReserveTransitionV1Parts {
    pub chain_id: u64,
    pub height: u64,
    pub parent_block_hash: Hash256,
    pub previous: Option<ReservePoolSnapshotV1>,
    pub native_deposit_total: u64,
    pub execution_withdrawal_total: u64,
    pub execution_fee_total: u64,
    pub new_execution_balance_total: u64,
    pub producer_coinbase_txid: Hash256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveTransitionV1 {
    parts: ReserveTransitionV1Parts,
    canonical_transition_bytes: Vec<u8>,
    transition_id: Hash256,
    new_reserve_amount: u64,
    new_reserve_outpoint: Option<OutPoint>,
}

impl ReserveTransitionV1 {
    pub fn new(parts: ReserveTransitionV1Parts) -> Result<Self, ReserveTransitionError> {
        if parts.previous.is_some_and(|previous| previous.amount == 0) {
            return Err(ReserveTransitionError::InvalidPreviousSnapshot);
        }

        let previous_amount = parts.previous.map_or(0, |previous| previous.amount);
        let after_deposit = previous_amount
            .checked_add(parts.native_deposit_total)
            .ok_or(ReserveTransitionError::ArithmeticOverflow)?;
        let after_withdrawal = after_deposit
            .checked_sub(parts.execution_withdrawal_total)
            .ok_or(ReserveTransitionError::ArithmeticUnderflow)?;
        let new_reserve_amount = after_withdrawal
            .checked_sub(parts.execution_fee_total)
            .ok_or(ReserveTransitionError::ArithmeticUnderflow)?;

        if new_reserve_amount != parts.new_execution_balance_total {
            return Err(ReserveTransitionError::ExecutionBalanceMismatch {
                reserve_amount: new_reserve_amount,
                execution_balance: parts.new_execution_balance_total,
            });
        }

        if let Some(previous) = parts.previous {
            Amount::from_base_units(previous.amount)
                .map_err(|_| ReserveTransitionError::AmountOutOfRange)?;
        }
        Amount::from_base_units(new_reserve_amount)
            .map_err(|_| ReserveTransitionError::AmountOutOfRange)?;

        let canonical_transition_bytes = encode_transition_parts(&parts);
        let transition_id = reserve_transition_id(&canonical_transition_bytes);
        let new_reserve_outpoint = (new_reserve_amount != 0).then_some(OutPoint {
            txid: reserve_outpoint_txid(transition_id),
            index: 0,
        });

        Ok(Self {
            parts,
            canonical_transition_bytes,
            transition_id,
            new_reserve_amount,
            new_reserve_outpoint,
        })
    }

    pub fn canonical_transition_bytes(&self) -> &[u8] {
        &self.canonical_transition_bytes
    }

    pub const fn transition_id(&self) -> Hash256 {
        self.transition_id
    }

    pub const fn new_reserve_amount(&self) -> u64 {
        self.new_reserve_amount
    }

    pub const fn new_reserve_outpoint(&self) -> Option<OutPoint> {
        self.new_reserve_outpoint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveUndoV1 {
    previous: Option<(OutPoint, UtxoEntry)>,
    created: Option<(OutPoint, UtxoEntry)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReserveTransitionError {
    #[error("reserve arithmetic overflow")]
    ArithmeticOverflow,
    #[error("reserve arithmetic underflow")]
    ArithmeticUnderflow,
    #[error(
        "reserve amount {reserve_amount} does not match execution balance {execution_balance}"
    )]
    ExecutionBalanceMismatch {
        reserve_amount: u64,
        execution_balance: u64,
    },
    #[error("previous reserve snapshot is invalid")]
    InvalidPreviousSnapshot,
    #[error("reserve amount exceeds the Oregon supply envelope")]
    AmountOutOfRange,
    #[error("previous reserve state does not match the transition")]
    PreviousReserveMismatch,
    #[error("multiple live execution reserve outputs exist")]
    MultipleLiveReserves,
    #[error("derived reserve outpoint collides with an existing UTXO: {0:?}")]
    OutputCollision(OutPoint),
    #[error("reserve undo does not match the current state")]
    UndoMismatch,
}

fn encode_transition_parts(parts: &ReserveTransitionV1Parts) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&RESERVE_TRANSITION_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&parts.chain_id.to_le_bytes());
    bytes.extend_from_slice(&parts.height.to_le_bytes());
    bytes.extend_from_slice(parts.parent_block_hash.as_bytes());

    match parts.previous {
        None => bytes.push(0x00),
        Some(previous) => {
            bytes.push(0x01);
            bytes.extend_from_slice(previous.outpoint.txid.as_bytes());
            bytes.extend_from_slice(&previous.outpoint.index.to_le_bytes());
        }
    }

    bytes.extend_from_slice(&parts.previous.map_or(0, |previous| previous.amount).to_le_bytes());
    bytes.extend_from_slice(&parts.native_deposit_total.to_le_bytes());
    bytes.extend_from_slice(&parts.execution_withdrawal_total.to_le_bytes());
    bytes.extend_from_slice(&parts.execution_fee_total.to_le_bytes());
    bytes.extend_from_slice(&parts.new_execution_balance_total.to_le_bytes());
    bytes.extend_from_slice(parts.producer_coinbase_txid.as_bytes());
    bytes
}

fn live_reserves(state: &UtxoState) -> Vec<(OutPoint, UtxoEntry)> {
    state
        .entries()
        .filter(|(_, entry)| {
            entry.output.locking_program.as_slice() == EXECUTION_RESERVE_LOCKING_PROGRAM_V1
        })
        .map(|(outpoint, entry)| (*outpoint, entry.clone()))
        .collect()
}

pub(crate) fn apply_reserve_transition_v1(
    state: &mut UtxoState,
    transition: &ReserveTransitionV1,
) -> Result<ReserveUndoV1, ReserveTransitionError> {
    let live = live_reserves(state);
    if live.len() > 1 {
        return Err(ReserveTransitionError::MultipleLiveReserves);
    }

    let previous = match transition.parts.previous {
        None => {
            if !live.is_empty() {
                return Err(ReserveTransitionError::PreviousReserveMismatch);
            }
            None
        }
        Some(expected) => {
            let Some((outpoint, entry)) = live.first() else {
                return Err(ReserveTransitionError::PreviousReserveMismatch);
            };
            if *outpoint != expected.outpoint
                || entry.output.value.base_units() != expected.amount
                || entry.output.locking_program.as_slice() != EXECUTION_RESERVE_LOCKING_PROGRAM_V1
            {
                return Err(ReserveTransitionError::PreviousReserveMismatch);
            }
            Some((*outpoint, entry.clone()))
        }
    };

    let created = match transition.new_reserve_outpoint {
        None => None,
        Some(outpoint) => {
            if state.get(&outpoint).is_some() {
                return Err(ReserveTransitionError::OutputCollision(outpoint));
            }
            let amount = Amount::from_base_units(transition.new_reserve_amount)
                .map_err(|_| ReserveTransitionError::AmountOutOfRange)?;
            Some((
                outpoint,
                UtxoEntry {
                    output: TxOutput {
                        value: amount,
                        locking_program: EXECUTION_RESERVE_LOCKING_PROGRAM_V1.to_vec(),
                    },
                    creation_height: transition.parts.height,
                    is_coinbase: false,
                },
            ))
        }
    };

    let mut overlay = state.clone();
    if let Some((outpoint, entry)) = &previous {
        if overlay.reserve_remove_entry(outpoint).as_ref() != Some(entry) {
            return Err(ReserveTransitionError::PreviousReserveMismatch);
        }
    }
    if let Some((outpoint, entry)) = &created {
        if overlay
            .reserve_insert_entry(*outpoint, entry.clone())
            .is_some()
        {
            return Err(ReserveTransitionError::OutputCollision(*outpoint));
        }
    }

    *state = overlay;
    Ok(ReserveUndoV1 { previous, created })
}

pub(crate) fn undo_reserve_transition_v1(
    state: &mut UtxoState,
    undo: &ReserveUndoV1,
) -> Result<(), ReserveTransitionError> {
    let live = live_reserves(state);
    match &undo.created {
        Some((expected_outpoint, expected_entry)) => {
            if live.len() != 1
                || live[0].0 != *expected_outpoint
                || live[0].1 != *expected_entry
                || state.get(expected_outpoint) != Some(expected_entry)
            {
                return Err(ReserveTransitionError::UndoMismatch);
            }
        }
        None => {
            if !live.is_empty() {
                return Err(ReserveTransitionError::UndoMismatch);
            }
        }
    }

    if let Some((previous_outpoint, _)) = &undo.previous {
        if state.get(previous_outpoint).is_some() {
            return Err(ReserveTransitionError::UndoMismatch);
        }
    }

    let mut overlay = state.clone();
    if let Some((outpoint, entry)) = &undo.created {
        if overlay.reserve_remove_entry(outpoint).as_ref() != Some(entry) {
            return Err(ReserveTransitionError::UndoMismatch);
        }
    }
    if let Some((outpoint, entry)) = &undo.previous {
        if overlay
            .reserve_insert_entry(*outpoint, entry.clone())
            .is_some()
        {
            return Err(ReserveTransitionError::UndoMismatch);
        }
    }

    *state = overlay;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ReservePoolSnapshotV1, ReserveTransitionError, ReserveTransitionV1,
        ReserveTransitionV1Parts, apply_reserve_transition_v1, undo_reserve_transition_v1,
    };
    use crate::{UtxoEntry, UtxoState};
    use oregon_primitives::execution_reserve::{
        EXECUTION_RESERVE_LOCKING_PROGRAM_V1, reserve_outpoint_txid, reserve_transition_id,
    };
    use oregon_primitives::{Amount, Hash256, OutPoint, TxOutput};

    fn outpoint(tag: u8, index: u32) -> OutPoint {
        OutPoint {
            txid: Hash256::from_bytes([tag; 32]),
            index,
        }
    }

    fn reserve_entry(amount: u64, locking_program: &[u8]) -> UtxoEntry {
        UtxoEntry {
            output: TxOutput {
                value: Amount::from_base_units(amount).unwrap(),
                locking_program: locking_program.to_vec(),
            },
            creation_height: 99,
            is_coinbase: false,
        }
    }

    fn state_with(entries: Vec<(OutPoint, UtxoEntry)>) -> UtxoState {
        UtxoState::try_from_entries(entries).unwrap()
    }

    fn parts(
        previous: u64,
        deposits: u64,
        withdrawals: u64,
        fees: u64,
        new_total: u64,
    ) -> ReserveTransitionV1Parts {
        ReserveTransitionV1Parts {
            chain_id: 7,
            height: 100,
            parent_block_hash: Hash256::from_bytes([0x10; 32]),
            previous: if previous == 0 {
                None
            } else {
                Some(ReservePoolSnapshotV1::test(previous))
            },
            native_deposit_total: deposits,
            execution_withdrawal_total: withdrawals,
            execution_fee_total: fees,
            new_execution_balance_total: new_total,
            producer_coinbase_txid: Hash256::from_bytes([0x20; 32]),
        }
    }

    fn reserve_count(state: &UtxoState) -> usize {
        state
            .entries()
            .filter(|(_, entry)| {
                entry.output.locking_program.as_slice() == EXECUTION_RESERVE_LOCKING_PROGRAM_V1
            })
            .count()
    }

    #[test]
    fn zero_to_deposit_creates_single_exact_reserve_and_forward_undo_forward_is_identical() {
        let transition = ReserveTransitionV1::new(parts(0, 100, 0, 0, 100)).unwrap();
        let mut state = UtxoState::new();
        let before = state.clone();

        let undo = apply_reserve_transition_v1(&mut state, &transition).unwrap();
        let first_forward = state.clone();
        let new_outpoint = transition.new_reserve_outpoint().unwrap();
        let entry = state.get(&new_outpoint).unwrap();
        assert_eq!(entry.output.value.base_units(), 100);
        assert_eq!(
            entry.output.locking_program.as_slice(),
            EXECUTION_RESERVE_LOCKING_PROGRAM_V1
        );
        assert_eq!(reserve_count(&state), 1);

        undo_reserve_transition_v1(&mut state, &undo).unwrap();
        assert_eq!(state, before);

        apply_reserve_transition_v1(&mut state, &transition).unwrap();
        assert_eq!(state, first_forward);
    }

    #[test]
    fn existing_reserve_can_increase_and_decrease_without_fragmentation() {
        let previous = ReservePoolSnapshotV1::test(100);
        let mut state = state_with(vec![(
            previous.outpoint,
            reserve_entry(100, EXECUTION_RESERVE_LOCKING_PROGRAM_V1),
        )]);
        let increase = ReserveTransitionV1::new(parts(100, 50, 0, 0, 150)).unwrap();
        apply_reserve_transition_v1(&mut state, &increase).unwrap();
        assert_eq!(reserve_count(&state), 1);
        let increased_outpoint = increase.new_reserve_outpoint().unwrap();
        assert_eq!(state.get(&increased_outpoint).unwrap().output.value.base_units(), 150);

        let decrease = ReserveTransitionV1::new(ReserveTransitionV1Parts {
            previous: Some(ReservePoolSnapshotV1 {
                outpoint: increased_outpoint,
                amount: 150,
            }),
            execution_withdrawal_total: 20,
            execution_fee_total: 30,
            new_execution_balance_total: 100,
            ..parts(0, 0, 0, 0, 0)
        })
        .unwrap();
        apply_reserve_transition_v1(&mut state, &decrease).unwrap();
        assert_eq!(reserve_count(&state), 1);
        assert_eq!(
            state
                .get(&decrease.new_reserve_outpoint().unwrap())
                .unwrap()
                .output
                .value
                .base_units(),
            100
        );
    }

    #[test]
    fn zero_result_removes_the_singleton_reserve() {
        let previous = ReservePoolSnapshotV1::test(100);
        let mut state = state_with(vec![(
            previous.outpoint,
            reserve_entry(100, EXECUTION_RESERVE_LOCKING_PROGRAM_V1),
        )]);
        let transition = ReserveTransitionV1::new(parts(100, 0, 80, 20, 0)).unwrap();

        apply_reserve_transition_v1(&mut state, &transition).unwrap();
        assert_eq!(transition.new_reserve_outpoint(), None);
        assert_eq!(reserve_count(&state), 0);
    }

    #[test]
    fn reserve_equation_rejects_underflow_overflow_and_total_mismatch() {
        assert_eq!(
            ReserveTransitionV1::new(parts(10, 0, 11, 0, 0)),
            Err(ReserveTransitionError::ArithmeticUnderflow)
        );
        assert_eq!(
            ReserveTransitionV1::new(parts(u64::MAX, 1, 0, 0, u64::MAX)),
            Err(ReserveTransitionError::ArithmeticOverflow)
        );
        assert_eq!(
            ReserveTransitionV1::new(parts(10, 5, 0, 0, 14)),
            Err(ReserveTransitionError::ExecutionBalanceMismatch {
                reserve_amount: 15,
                execution_balance: 14,
            })
        );
    }

    #[test]
    fn wrong_previous_program_or_outpoint_fails_closed_without_mutation() {
        let expected = ReservePoolSnapshotV1::test(100);
        let transition = ReserveTransitionV1::new(parts(100, 1, 0, 0, 101)).unwrap();

        let mut wrong_program = state_with(vec![(
            expected.outpoint,
            reserve_entry(100, b"not-the-reserve-program"),
        )]);
        let before = wrong_program.clone();
        assert_eq!(
            apply_reserve_transition_v1(&mut wrong_program, &transition),
            Err(ReserveTransitionError::PreviousReserveMismatch)
        );
        assert_eq!(wrong_program, before);

        let mut wrong_outpoint = state_with(vec![(
            outpoint(0x91, 0),
            reserve_entry(100, EXECUTION_RESERVE_LOCKING_PROGRAM_V1),
        )]);
        let before = wrong_outpoint.clone();
        assert_eq!(
            apply_reserve_transition_v1(&mut wrong_outpoint, &transition),
            Err(ReserveTransitionError::PreviousReserveMismatch)
        );
        assert_eq!(wrong_outpoint, before);
    }

    #[test]
    fn stale_or_multiple_live_reserve_state_fails_closed() {
        let transition = ReserveTransitionV1::new(parts(100, 1, 0, 0, 101)).unwrap();
        let mut missing = UtxoState::new();
        assert_eq!(
            apply_reserve_transition_v1(&mut missing, &transition),
            Err(ReserveTransitionError::PreviousReserveMismatch)
        );

        let previous = ReservePoolSnapshotV1::test(100);
        let mut multiple = state_with(vec![
            (
                previous.outpoint,
                reserve_entry(100, EXECUTION_RESERVE_LOCKING_PROGRAM_V1),
            ),
            (
                outpoint(0x92, 0),
                reserve_entry(1, EXECUTION_RESERVE_LOCKING_PROGRAM_V1),
            ),
        ]);
        let before = multiple.clone();
        assert_eq!(
            apply_reserve_transition_v1(&mut multiple, &transition),
            Err(ReserveTransitionError::MultipleLiveReserves)
        );
        assert_eq!(multiple, before);
    }

    #[test]
    fn transition_preimage_and_new_outpoint_are_exact() {
        let transition = ReserveTransitionV1::new(parts(0, 40, 0, 0, 40)).unwrap();
        let mut expected = Vec::new();
        expected.extend_from_slice(&1u16.to_le_bytes());
        expected.extend_from_slice(&7u64.to_le_bytes());
        expected.extend_from_slice(&100u64.to_le_bytes());
        expected.extend_from_slice(Hash256::from_bytes([0x10; 32]).as_bytes());
        expected.push(0x00);
        expected.extend_from_slice(&0u64.to_le_bytes());
        expected.extend_from_slice(&40u64.to_le_bytes());
        expected.extend_from_slice(&0u64.to_le_bytes());
        expected.extend_from_slice(&0u64.to_le_bytes());
        expected.extend_from_slice(&40u64.to_le_bytes());
        expected.extend_from_slice(Hash256::from_bytes([0x20; 32]).as_bytes());

        assert_eq!(transition.canonical_transition_bytes(), expected.as_slice());
        let expected_id = reserve_transition_id(&expected);
        assert_eq!(transition.transition_id(), expected_id);
        assert_eq!(
            transition.new_reserve_outpoint(),
            Some(OutPoint {
                txid: reserve_outpoint_txid(expected_id),
                index: 0,
            })
        );
    }

    #[test]
    fn post_state_roots_are_not_inputs_to_transition_identity() {
        let first_fake_root = Hash256::from_bytes([0xa1; 32]);
        let second_fake_root = Hash256::from_bytes([0xa2; 32]);
        assert_ne!(first_fake_root, second_fake_root);

        let first = ReserveTransitionV1::new(parts(0, 25, 0, 0, 25)).unwrap();
        let second = ReserveTransitionV1::new(parts(0, 25, 0, 0, 25)).unwrap();
        assert_eq!(first.transition_id(), second.transition_id());
        assert_eq!(
            first.canonical_transition_bytes(),
            second.canonical_transition_bytes()
        );
    }

    #[test]
    fn derived_outpoint_collision_is_rejected_atomically() {
        let transition = ReserveTransitionV1::new(parts(0, 10, 0, 0, 10)).unwrap();
        let collision = transition.new_reserve_outpoint().unwrap();
        let mut state = state_with(vec![(collision, reserve_entry(1, b"ordinary"))]);
        let before = state.clone();

        assert_eq!(
            apply_reserve_transition_v1(&mut state, &transition),
            Err(ReserveTransitionError::OutputCollision(collision))
        );
        assert_eq!(state, before);
    }

    #[test]
    fn undo_rejects_tampered_forward_state() {
        let transition = ReserveTransitionV1::new(parts(0, 10, 0, 0, 10)).unwrap();
        let mut state = UtxoState::new();
        let undo = apply_reserve_transition_v1(&mut state, &transition).unwrap();
        let created = transition.new_reserve_outpoint().unwrap();
        let mut tampered_entries: Vec<_> = state
            .entries()
            .map(|(outpoint, entry)| (*outpoint, entry.clone()))
            .collect();
        tampered_entries.retain(|(outpoint, _)| *outpoint != created);
        tampered_entries.push((
            created,
            reserve_entry(9, EXECUTION_RESERVE_LOCKING_PROGRAM_V1),
        ));
        let mut tampered = state_with(tampered_entries);
        let before = tampered.clone();

        assert_eq!(
            undo_reserve_transition_v1(&mut tampered, &undo),
            Err(ReserveTransitionError::UndoMismatch)
        );
        assert_eq!(tampered, before);
    }
}
