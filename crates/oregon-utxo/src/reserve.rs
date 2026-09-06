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
