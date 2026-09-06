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

mod meter {
    use oregon_primitives::Hash256;
    use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
    use oregon_primitives::execution_envelope::ExecutionDomain;
    use oregon_primitives::execution_event::MAX_EXECUTION_EVENTS_V1;
    use oregon_runtime::{
        RuntimeBackendFailureV1, RuntimeBackendV1, RuntimeCallContextV1,
        RuntimeCallContextV1Parts, RuntimeCallResultV1, RuntimeHostSignalV1, RuntimeHostV1,
    };

    use crate::{MeterScheduleV1, ResourceDomain, WeightMeter, WeightRatio};

    use super::super::effects::EffectStackV1;
    use super::super::host::{
        CoordinatorHostV1, HostChargeScheduleV1, HostChargeScheduleV1Parts,
    };
    use super::super::types::{CoordinatorLimitsV1, CoordinatorTerminalV1};

    fn address(kind: ExecutionAddressKind, byte: u8) -> ExecutionAddress {
        match kind {
            ExecutionAddressKind::Evm => ExecutionAddress::from_evm([byte; 20]),
            _ => ExecutionAddress::new(kind, [byte; 32]).unwrap(),
        }
    }

    fn context(domain: ExecutionDomain) -> RuntimeCallContextV1 {
        let target_kind = match domain {
            ExecutionDomain::Evm => ExecutionAddressKind::Evm,
            ExecutionDomain::Wasm => ExecutionAddressKind::Wasm,
            _ => panic!("test helper supports active VM domains only"),
        };
        RuntimeCallContextV1::from_trusted_parts(RuntimeCallContextV1Parts {
            chain_id: 42,
            height: 7,
            parent_block_hash: Hash256::from_bytes([0x11; 32]),
            txid: Hash256::from_bytes([0x22; 32]),
            principal: address(ExecutionAddressKind::Wasm, 0x33),
            caller: address(ExecutionAddressKind::Wasm, 0x44),
            target: address(target_kind, 0x55),
            execution_domain: domain,
            depth: 1,
            read_only: false,
            transferred_value: 0,
        })
        .unwrap()
    }

    fn meter_schedule(evm_num: u64, wasm_num: u64) -> MeterScheduleV1 {
        MeterScheduleV1::new(
            1,
            WeightRatio::new(1, 1).unwrap(),
            WeightRatio::new(evm_num, 1).unwrap(),
            WeightRatio::new(wasm_num, 1).unwrap(),
        )
        .unwrap()
    }

    fn effects() -> EffectStackV1 {
        EffectStackV1::new(
            CoordinatorLimitsV1::new(64, MAX_EXECUTION_EVENTS_V1, 2_097_152).unwrap(),
        )
    }

    fn host_charges(event_base: u64, event_data_byte: u64, context_query: u64) -> HostChargeScheduleV1 {
        HostChargeScheduleV1::new(HostChargeScheduleV1Parts {
            version: 1,
            state_read_base: 1,
            state_read_key_byte: 1,
            state_read_copy_byte: 1,
            state_write_base: 1,
            state_write_key_byte: 1,
            state_write_value_byte: 1,
            state_delete_base: 1,
            state_delete_key_byte: 1,
            event_base,
            event_topic: 1,
            event_data_byte,
            nested_call_base: 1,
            nested_call_input_byte: 1,
            nested_call_return_copy_byte: 1,
            context_query,
        })
        .unwrap()
    }

    #[test]
    fn backend_cannot_choose_a_cheaper_resource_domain() {
        let context = context(ExecutionDomain::Evm);
        let mut meter = WeightMeter::new(meter_schedule(2, 1), 100, 0).unwrap();
        let mut effects = effects();
        let mut terminal = CoordinatorTerminalV1::Running;
        let mut host = CoordinatorHostV1::new(
            &context,
            &mut meter,
            &mut effects,
            &mut terminal,
            host_charges(1, 1, 1),
        );

        host.charge_vm_units(3).unwrap();
        drop(host);
        assert_eq!(meter.consumed(), 6);
    }

    #[test]
    fn rollback_does_not_refund_the_shared_meter() {
        let context = context(ExecutionDomain::Wasm);
        let mut meter = WeightMeter::new(meter_schedule(1, 1), 100, 0).unwrap();
        let mut effects = effects();
        let mut terminal = CoordinatorTerminalV1::Running;
        let mut host = CoordinatorHostV1::new(
            &context,
            &mut meter,
            &mut effects,
            &mut terminal,
            host_charges(1, 1, 1),
        );
        host.charge_vm_units(7).unwrap();
        drop(host);

        effects.begin().unwrap();
        effects.revert().unwrap();
        assert_eq!(meter.consumed(), 7);
    }

    #[test]
    fn common_host_work_and_vm_work_share_one_budget() {
        let context = context(ExecutionDomain::Wasm);
        let mut meter = WeightMeter::new(meter_schedule(1, 1), 10, 0).unwrap();
        let mut effects = effects();
        let mut terminal = CoordinatorTerminalV1::Running;
        let mut host = CoordinatorHostV1::new(
            &context,
            &mut meter,
            &mut effects,
            &mut terminal,
            host_charges(1, 1, 1),
        );

        host.charge_common(3).unwrap();
        host.charge_vm_units(2).unwrap();
        drop(host);
        assert_eq!(meter.consumed(), 5);
    }

    #[test]
    fn exact_budget_succeeds_then_one_over_latches_sticky_exhaustion() {
        let context = context(ExecutionDomain::Wasm);
        let mut meter = WeightMeter::new(meter_schedule(1, 1), 10, 0).unwrap();
        let mut effects = effects();
        let mut terminal = CoordinatorTerminalV1::Running;
        let mut host = CoordinatorHostV1::new(
            &context,
            &mut meter,
            &mut effects,
            &mut terminal,
            host_charges(1, 1, 1),
        );

        host.charge_common(10).unwrap();
        assert_eq!(host.charge_common(1), Err(RuntimeHostSignalV1::Abort));
        assert_eq!(host.charge_vm_units(1), Err(RuntimeHostSignalV1::Abort));
        drop(host);

        assert_eq!(meter.consumed(), 10);
        assert!(meter.is_exhausted());
        assert_eq!(terminal, CoordinatorTerminalV1::ResourceExhausted);
    }

    struct CatchAbortAndReturnSuccess;

    impl RuntimeBackendV1 for CatchAbortAndReturnSuccess {
        fn execute(
            &mut self,
            host: &mut dyn RuntimeHostV1,
        ) -> Result<RuntimeCallResultV1, RuntimeBackendFailureV1> {
            let _ = host.charge_common(11);
            Ok(RuntimeCallResultV1::Success(Vec::new()))
        }
    }

    #[test]
    fn backend_success_cannot_mask_latched_exhaustion() {
        let context = context(ExecutionDomain::Wasm);
        let mut meter = WeightMeter::new(meter_schedule(1, 1), 10, 0).unwrap();
        let mut effects = effects();
        let mut terminal = CoordinatorTerminalV1::Running;
        let mut host = CoordinatorHostV1::new(
            &context,
            &mut meter,
            &mut effects,
            &mut terminal,
            host_charges(1, 1, 1),
        );
        let mut backend = CatchAbortAndReturnSuccess;

        let result = backend.execute(&mut host).unwrap();
        assert!(matches!(result, RuntimeCallResultV1::Success(_)));
        drop(host);

        assert_eq!(terminal, CoordinatorTerminalV1::ResourceExhausted);
        assert_eq!(meter.consumed(), 10);
    }

    #[test]
    fn event_input_charge_happens_before_any_effect_copy() {
        let context = context(ExecutionDomain::Wasm);
        let mut meter = WeightMeter::new(meter_schedule(1, 1), 5, 0).unwrap();
        let mut effects = effects();
        let mut terminal = CoordinatorTerminalV1::Running;
        let mut host = CoordinatorHostV1::new(
            &context,
            &mut meter,
            &mut effects,
            &mut terminal,
            host_charges(1, 1, 1),
        );

        assert_eq!(
            host.emit_event(&[], &[0u8; 5]),
            Err(RuntimeHostSignalV1::Abort)
        );
        drop(host);

        assert_eq!(terminal, CoordinatorTerminalV1::ResourceExhausted);
        assert_eq!(meter.consumed(), 5);
        assert!(effects.root_events().is_empty());
        assert_eq!(effects.live_retained_bytes(), 0);
    }

    #[test]
    fn context_query_uses_the_same_common_meter() {
        let context = context(ExecutionDomain::Wasm);
        let mut meter = WeightMeter::new(meter_schedule(1, 1), 10, 0).unwrap();
        let mut effects = effects();
        let mut terminal = CoordinatorTerminalV1::Running;
        let mut host = CoordinatorHostV1::new(
            &context,
            &mut meter,
            &mut effects,
            &mut terminal,
            host_charges(1, 1, 2),
        );

        host.charge_context_query().unwrap();
        drop(host);
        assert_eq!(meter.consumed(), 2);
    }

    #[test]
    fn host_charge_overflow_latches_fatal_instead_of_wrapping() {
        let context = context(ExecutionDomain::Wasm);
        let mut meter = WeightMeter::new(meter_schedule(1, 1), 100, 0).unwrap();
        let mut effects = effects();
        let mut terminal = CoordinatorTerminalV1::Running;
        let mut host = CoordinatorHostV1::new(
            &context,
            &mut meter,
            &mut effects,
            &mut terminal,
            host_charges(1, u64::MAX, 1),
        );

        assert_eq!(
            host.emit_event(&[], &[0u8; 2]),
            Err(RuntimeHostSignalV1::Abort)
        );
        drop(host);

        assert_eq!(terminal, CoordinatorTerminalV1::Fatal);
        assert_eq!(meter.consumed(), 0);
        assert!(effects.root_events().is_empty());
    }

    #[test]
    fn current_vm_domain_mapping_is_exact() {
        assert_eq!(ResourceDomain::Evm as u8, 1);
        assert_eq!(ResourceDomain::Wasm as u8, 2);
    }
}
