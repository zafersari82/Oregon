mod effects {
    use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
    use oregon_primitives::execution_event::{ExecutionEventV1, MAX_EXECUTION_EVENTS_V1};

    use super::super::effects::EffectStackV1;
    use super::super::types::{CoordinatorError, CoordinatorLimitsV1, CoordinatorTerminalV1};

    const MAX_EFFECT_BYTES: usize = 2_097_152;

    fn limits() -> CoordinatorLimitsV1 {
        CoordinatorLimitsV1::new(64, MAX_EXECUTION_EVENTS_V1, MAX_EFFECT_BYTES).unwrap()
    }

    fn emitter() -> ExecutionAddress {
        ExecutionAddress::new(ExecutionAddressKind::Wasm, [0x44; 32]).unwrap()
    }

    fn event_with_data(bytes: usize) -> ExecutionEventV1 {
        ExecutionEventV1::new(emitter(), Vec::new(), vec![0x5a; bytes]).unwrap()
    }

    #[test]
    fn root_frame_cannot_be_committed_or_reverted_as_a_child() {
        let mut effects = EffectStackV1::new(limits());
        assert_eq!(effects.commit(), Err(CoordinatorError::RootFrameLifecycle));
        assert_eq!(effects.revert(), Err(CoordinatorError::RootFrameLifecycle));
        assert_eq!(effects.depth(), 1);
    }

    #[test]
    fn child_success_merges_events_into_parent_in_order() {
        let mut effects = EffectStackV1::new(limits());
        let first = event_with_data(3);
        let second = event_with_data(5);

        effects.push_event(first.clone()).unwrap();
        effects.begin().unwrap();
        effects.push_event(second.clone()).unwrap();
        effects.commit().unwrap();

        assert_eq!(effects.root_events(), &[first, second]);
        assert_eq!(effects.live_retained_bytes(), 8);
    }

    #[test]
    fn reverted_child_releases_effect_bytes_but_not_frame_history() {
        let mut effects = EffectStackV1::new(limits());
        effects.begin().unwrap();
        effects.push_event(event_with_data(1024)).unwrap();
        effects.retain_return_bytes(512).unwrap();
        effects.revert().unwrap();

        assert_eq!(effects.live_retained_bytes(), 0);
        assert_eq!(effects.total_frames_created(), 2);
        assert!(effects.root_events().is_empty());
    }

    #[test]
    fn ancestor_revert_discards_committed_descendant_effects() {
        let mut effects = EffectStackV1::new(limits());
        effects.begin().unwrap();
        effects.begin().unwrap();
        effects.push_event(event_with_data(17)).unwrap();
        effects.retain_return_bytes(19).unwrap();
        effects.commit().unwrap();

        assert_eq!(effects.live_retained_bytes(), 36);
        effects.revert().unwrap();

        assert_eq!(effects.live_retained_bytes(), 0);
        assert!(effects.root_events().is_empty());
        assert_eq!(effects.total_frames_created(), 3);
    }

    #[test]
    fn event_count_accepts_exact_limit_and_rejects_one_over_atomically() {
        let mut effects = EffectStackV1::new(limits());
        for _ in 0..MAX_EXECUTION_EVENTS_V1 {
            effects.push_event(event_with_data(0)).unwrap();
        }
        assert_eq!(effects.root_events().len(), MAX_EXECUTION_EVENTS_V1);

        assert_eq!(
            effects.push_event(event_with_data(0)),
            Err(CoordinatorError::EventLimitExceeded)
        );
        assert_eq!(effects.root_events().len(), MAX_EXECUTION_EVENTS_V1);
    }

    #[test]
    fn aggregate_retained_bytes_accept_exact_limit_and_reject_one_over_atomically() {
        let mut effects = EffectStackV1::new(limits());
        effects.retain_return_bytes(MAX_EFFECT_BYTES).unwrap();
        assert_eq!(effects.live_retained_bytes(), MAX_EFFECT_BYTES);

        assert_eq!(
            effects.retain_return_bytes(1),
            Err(CoordinatorError::EffectBytesLimitExceeded)
        );
        assert_eq!(effects.live_retained_bytes(), MAX_EFFECT_BYTES);
    }

    #[test]
    fn oversized_event_data_is_rejected_before_effect_retention() {
        let mut effects = EffectStackV1::new(limits());
        let oversized = vec![0u8; 65_537];
        assert_eq!(
            effects.push_event_parts(emitter(), &[], &oversized),
            Err(CoordinatorError::EventDataTooLarge)
        );
        assert_eq!(effects.live_retained_bytes(), 0);
        assert!(effects.root_events().is_empty());
    }

    #[test]
    fn frame_stack_mismatch_latches_fatal_and_cannot_be_cleared() {
        let mut terminal = CoordinatorTerminalV1::Running;
        let mut effects = EffectStackV1::new(limits());
        effects.begin().unwrap();

        assert_eq!(
            effects.assert_depth_matches(1, &mut terminal),
            Err(CoordinatorError::FrameStackMismatch)
        );
        assert_eq!(terminal, CoordinatorTerminalV1::Fatal);

        effects.assert_depth_matches(2, &mut terminal).unwrap();
        assert_eq!(terminal, CoordinatorTerminalV1::Fatal);
    }
}
