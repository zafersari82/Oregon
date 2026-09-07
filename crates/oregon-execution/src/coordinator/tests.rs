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
        RuntimeBackendFailureV1, RuntimeBackendV1, RuntimeCallContextV1, RuntimeCallContextV1Parts,
        RuntimeCallResultV1, RuntimeHostSignalV1, RuntimeHostV1,
    };

    use crate::{MeterScheduleV1, ResourceDomain, WeightMeter, WeightRatio};

    use super::super::effects::EffectStackV1;
    use super::super::host::{CoordinatorHostV1, HostChargeScheduleV1, HostChargeScheduleV1Parts};
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

    fn host_charges(
        event_base: u64,
        event_data_byte: u64,
        context_query: u64,
    ) -> HostChargeScheduleV1 {
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
        {
            let mut host = CoordinatorHostV1::new(
                &context,
                &mut meter,
                &mut effects,
                &mut terminal,
                host_charges(1, 1, 1),
            );
            host.charge_vm_units(3).unwrap();
        }
        assert_eq!(meter.consumed(), 6);
    }

    #[test]
    fn rollback_does_not_refund_the_shared_meter() {
        let context = context(ExecutionDomain::Wasm);
        let mut meter = WeightMeter::new(meter_schedule(1, 1), 100, 0).unwrap();
        let mut effects = effects();
        let mut terminal = CoordinatorTerminalV1::Running;
        {
            let mut host = CoordinatorHostV1::new(
                &context,
                &mut meter,
                &mut effects,
                &mut terminal,
                host_charges(1, 1, 1),
            );
            host.charge_vm_units(7).unwrap();
        }

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
        {
            let mut host = CoordinatorHostV1::new(
                &context,
                &mut meter,
                &mut effects,
                &mut terminal,
                host_charges(1, 1, 1),
            );
            host.charge_common(3).unwrap();
            host.charge_vm_units(2).unwrap();
        }
        assert_eq!(meter.consumed(), 5);
    }

    #[test]
    fn exact_budget_succeeds_then_one_over_latches_sticky_exhaustion() {
        let context = context(ExecutionDomain::Wasm);
        let mut meter = WeightMeter::new(meter_schedule(1, 1), 10, 0).unwrap();
        let mut effects = effects();
        let mut terminal = CoordinatorTerminalV1::Running;
        {
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
        }

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
        let mut backend = CatchAbortAndReturnSuccess;

        let result = {
            let mut host = CoordinatorHostV1::new(
                &context,
                &mut meter,
                &mut effects,
                &mut terminal,
                host_charges(1, 1, 1),
            );
            backend.execute(&mut host).unwrap()
        };
        assert!(matches!(result, RuntimeCallResultV1::Success(_)));

        assert_eq!(terminal, CoordinatorTerminalV1::ResourceExhausted);
        assert_eq!(meter.consumed(), 10);
    }

    #[test]
    fn event_input_charge_happens_before_any_effect_copy() {
        let context = context(ExecutionDomain::Wasm);
        let mut meter = WeightMeter::new(meter_schedule(1, 1), 5, 0).unwrap();
        let mut effects = effects();
        let mut terminal = CoordinatorTerminalV1::Running;
        {
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
        }

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
        {
            let mut host = CoordinatorHostV1::new(
                &context,
                &mut meter,
                &mut effects,
                &mut terminal,
                host_charges(1, 1, 2),
            );
            host.charge_context_query().unwrap();
        }
        assert_eq!(meter.consumed(), 2);
    }

    #[test]
    fn host_charge_overflow_latches_fatal_instead_of_wrapping() {
        let context = context(ExecutionDomain::Wasm);
        let mut meter = WeightMeter::new(meter_schedule(1, 1), 100, 0).unwrap();
        let mut effects = effects();
        let mut terminal = CoordinatorTerminalV1::Running;
        {
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
        }

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

mod settlement {
    use oregon_contract_state::{
        DomainSnapshot, StateError, StateNode, StateSource, empty_hashes, encode_accounting_u64,
        total_execution_balance_key,
    };
    use oregon_primitives::Hash256;
    use oregon_primitives::execution_address::{ExecutionAddress, ExecutionAddressKind};
    use oregon_primitives::execution_envelope::ExecutionDomain;
    use oregon_primitives::execution_event::{ExecutionEventV1, MAX_EXECUTION_EVENTS_V1};
    use oregon_primitives::fee_settlement::{ExecutionOutcome, FeeSourceKind};
    use oregon_primitives::state_commitment::CommitmentDomainId;
    use oregon_runtime::{RuntimeCallResultV1, RuntimeTrapCodeV1};

    use crate::{
        EscrowBookV1, ExecutionJournalV1, FeeTermsV1, FundingCapabilityV1, JournalContextV1,
        JournalLimitsV1, MeterScheduleV1, WeightMeter, WeightRatio,
    };

    use super::super::accounting::{read_balance, read_total_execution_balance, write_balance};
    use super::super::effects::EffectStackV1;
    use super::super::settlement::{
        FundingSourceValidatorV1, FundingValidationRequestV1, open_validated_escrow,
        settle_top_level_execution,
    };
    use super::super::types::{
        CoordinatorError, CoordinatorLimitsV1, CoordinatorOutcomeV1, CoordinatorSettlementV1,
        CoordinatorTerminalV1,
    };

    #[derive(Debug, Default)]
    struct EmptySource;

    impl StateSource for EmptySource {
        fn get_node(&self, _node_hash: &Hash256) -> Result<Option<StateNode>, StateError> {
            Ok(None)
        }

        fn get_value(&self, _value_hash: &Hash256) -> Result<Option<Vec<u8>>, StateError> {
            Ok(None)
        }
    }

    fn address(kind: ExecutionAddressKind, byte: u8) -> ExecutionAddress {
        match kind {
            ExecutionAddressKind::Evm => ExecutionAddress::from_evm([byte; 20]),
            _ => ExecutionAddress::new(kind, [byte; 32]).unwrap(),
        }
    }

    fn payer() -> ExecutionAddress {
        address(ExecutionAddressKind::Wasm, 0x77)
    }

    fn target() -> ExecutionAddress {
        address(ExecutionAddressKind::Wasm, 0x88)
    }

    fn journal(source: &EmptySource) -> ExecutionJournalV1<'_, EmptySource> {
        let domain = CommitmentDomainId::ExecutionAccounting;
        ExecutionJournalV1::new(
            source,
            JournalContextV1 {
                chain_id: 42,
                height: 9001,
                parent_block_hash: Hash256::from_bytes([0x11; 32]),
                txid: Hash256::from_bytes([0x22; 32]),
            },
            &[DomainSnapshot {
                domain,
                root: empty_hashes(domain)[0],
            }],
            JournalLimitsV1::default(),
        )
        .unwrap()
    }

    fn effects() -> EffectStackV1 {
        EffectStackV1::new(
            CoordinatorLimitsV1::new(64, MAX_EXECUTION_EVENTS_V1, 2_097_152).unwrap(),
        )
    }

    fn seed_accounting(journal: &mut ExecutionJournalV1<'_, EmptySource>, value: u64) {
        write_balance(journal, payer(), value).unwrap();
        journal
            .put(
                CommitmentDomainId::ExecutionAccounting,
                total_execution_balance_key(),
                &encode_accounting_u64(value),
            )
            .unwrap();
    }

    fn terms() -> FeeTermsV1 {
        FeeTermsV1::new(1, 4, 1, 10).unwrap()
    }

    fn request(source_kind: FeeSourceKind) -> FundingValidationRequestV1 {
        FundingValidationRequestV1 {
            chain_id: 42,
            height: 9001,
            txid: Hash256::from_bytes([0x22; 32]),
            execution_domain: ExecutionDomain::Wasm,
            principal: address(ExecutionAddressKind::Wasm, 0x66),
            payer: payer(),
            authorization_commitment: Hash256::from_bytes([0x33; 32]),
            source_kind,
            source_commitment: Hash256::from_bytes([0x44; 32]),
            source_sequence: 7,
            fee_terms: terms(),
        }
    }

    #[derive(Debug)]
    struct Validator {
        capability: FundingCapabilityV1,
    }

    impl FundingSourceValidatorV1 for Validator {
        fn validate(
            &mut self,
            _request: &FundingValidationRequestV1,
        ) -> Result<FundingCapabilityV1, CoordinatorError> {
            Ok(self.capability)
        }
    }

    fn validator(source_kind: FeeSourceKind, available_amount: u64) -> Validator {
        let request = request(source_kind);
        Validator {
            capability: FundingCapabilityV1::new(
                source_kind,
                request.payer,
                request.source_commitment,
                available_amount,
                request.source_sequence,
                request.authorization_commitment,
            )
            .unwrap(),
        }
    }

    fn open_ticket(
        journal: &mut ExecutionJournalV1<'_, EmptySource>,
        book: &mut EscrowBookV1,
        source_kind: FeeSourceKind,
    ) -> crate::EscrowTicketV1 {
        let request = request(source_kind);
        let mut validator = validator(source_kind, 100);
        open_validated_escrow(journal, book, &mut validator, request).unwrap()
    }

    fn meter(actual_weight: u64) -> WeightMeter {
        let schedule = MeterScheduleV1::new(
            1,
            WeightRatio::new(1, 1).unwrap(),
            WeightRatio::new(1, 1).unwrap(),
            WeightRatio::new(1, 1).unwrap(),
        )
        .unwrap();
        WeightMeter::new(schedule, 10, actual_weight).unwrap()
    }

    fn stage_top_level_child(
        journal: &mut ExecutionJournalV1<'_, EmptySource>,
        effects: &mut EffectStackV1,
    ) {
        journal.begin_frame().unwrap();
        effects.begin().unwrap();
        write_balance(journal, target(), 7).unwrap();
        effects
            .push_event(ExecutionEventV1::new(target(), Vec::new(), vec![0x5a]).unwrap())
            .unwrap();
    }

    fn run_case(
        runtime_result: RuntimeCallResultV1,
    ) -> (CoordinatorSettlementV1, u64, u64, u64, usize) {
        let source = EmptySource;
        let mut journal = journal(&source);
        seed_accounting(&mut journal, 100);
        let mut book = EscrowBookV1::new(4).unwrap();
        let ticket = open_ticket(&mut journal, &mut book, FeeSourceKind::ExecutionBalance);
        let mut effects = effects();
        stage_top_level_child(&mut journal, &mut effects);
        let meter = meter(5);
        let mut terminal = CoordinatorTerminalV1::Running;

        let settlement = settle_top_level_execution(
            &mut journal,
            &mut effects,
            &mut book,
            ticket,
            &meter,
            &mut terminal,
            runtime_result,
        )
        .unwrap();

        (
            settlement,
            read_balance(&journal, payer()).unwrap(),
            read_total_execution_balance(&journal).unwrap(),
            read_balance(&journal, target()).unwrap(),
            effects.root_events().len(),
        )
    }

    #[test]
    fn committed_reverted_and_trapped_equal_weight_charge_equally_and_map_frames() {
        let (committed, committed_payer, committed_total, committed_target, committed_events) =
            run_case(RuntimeCallResultV1::Success(Vec::new()));
        let (reverted, reverted_payer, reverted_total, reverted_target, reverted_events) =
            run_case(RuntimeCallResultV1::Revert(Vec::new()));
        let (trapped, trapped_payer, trapped_total, trapped_target, trapped_events) = run_case(
            RuntimeCallResultV1::trap(RuntimeTrapCodeV1::ReadOnlyViolation),
        );

        assert_eq!(committed.outcome(), CoordinatorOutcomeV1::Committed);
        assert_eq!(reverted.outcome(), CoordinatorOutcomeV1::Reverted);
        assert_eq!(
            trapped.outcome(),
            CoordinatorOutcomeV1::Trapped(RuntimeTrapCodeV1::ReadOnlyViolation)
        );
        assert_eq!(
            committed.fee_receipt().outcome(),
            ExecutionOutcome::Committed
        );
        assert_eq!(reverted.fee_receipt().outcome(), ExecutionOutcome::Reverted);
        assert_eq!(trapped.fee_receipt().outcome(), ExecutionOutcome::Reverted);
        assert_eq!(committed.fee_receipt().charged(), 10);
        assert_eq!(reverted.fee_receipt().charged(), 10);
        assert_eq!(trapped.fee_receipt().charged(), 10);
        assert_eq!((committed_payer, committed_total), (90, 90));
        assert_eq!((reverted_payer, reverted_total), (90, 90));
        assert_eq!((trapped_payer, trapped_total), (90, 90));
        assert_eq!((committed_target, committed_events), (7, 1));
        assert_eq!((reverted_target, reverted_events), (0, 0));
        assert_eq!((trapped_target, trapped_events), (0, 0));
    }

    #[test]
    fn resource_exhaustion_forces_full_weight_revert_and_cannot_be_masked_by_success() {
        let source = EmptySource;
        let mut journal = journal(&source);
        seed_accounting(&mut journal, 100);
        let mut book = EscrowBookV1::new(4).unwrap();
        let ticket = open_ticket(&mut journal, &mut book, FeeSourceKind::ExecutionBalance);
        let mut effects = effects();
        stage_top_level_child(&mut journal, &mut effects);
        let mut meter = meter(0);
        assert!(meter.charge_common(11).is_err());
        let mut terminal = CoordinatorTerminalV1::ResourceExhausted;

        let settlement = settle_top_level_execution(
            &mut journal,
            &mut effects,
            &mut book,
            ticket,
            &meter,
            &mut terminal,
            RuntimeCallResultV1::Success(Vec::new()),
        )
        .unwrap();

        assert_eq!(
            settlement.outcome(),
            CoordinatorOutcomeV1::ResourceExhausted
        );
        assert_eq!(
            settlement.fee_receipt().outcome(),
            ExecutionOutcome::ResourceExhausted
        );
        assert_eq!(settlement.fee_receipt().actual_weight(), 10);
        assert_eq!(settlement.fee_receipt().charged(), 20);
        assert_eq!(read_balance(&journal, payer()).unwrap(), 80);
        assert_eq!(read_total_execution_balance(&journal).unwrap(), 80);
        assert_eq!(read_balance(&journal, target()).unwrap(), 0);
        assert!(effects.root_events().is_empty());
    }

    #[test]
    fn native_funded_settlement_does_not_touch_execution_accounting() {
        let source = EmptySource;
        let mut journal = journal(&source);
        seed_accounting(&mut journal, 100);
        let mut book = EscrowBookV1::new(4).unwrap();
        let ticket = open_ticket(&mut journal, &mut book, FeeSourceKind::NativeUtxo);
        let mut effects = effects();
        journal.begin_frame().unwrap();
        effects.begin().unwrap();
        let meter = meter(5);
        let mut terminal = CoordinatorTerminalV1::Running;

        let settlement = settle_top_level_execution(
            &mut journal,
            &mut effects,
            &mut book,
            ticket,
            &meter,
            &mut terminal,
            RuntimeCallResultV1::Success(Vec::new()),
        )
        .unwrap();

        assert_eq!(settlement.fee_receipt().charged(), 10);
        assert_eq!(read_balance(&journal, payer()).unwrap(), 100);
        assert_eq!(read_total_execution_balance(&journal).unwrap(), 100);
    }

    #[test]
    fn pre_escrow_invalidity_consumes_no_source_and_creates_no_settlement_path() {
        let source = EmptySource;
        let mut journal = journal(&source);
        seed_accounting(&mut journal, 100);
        let mut book = EscrowBookV1::new(4).unwrap();
        let request = request(FeeSourceKind::ExecutionBalance);
        let mut insufficient = validator(FeeSourceKind::ExecutionBalance, 39);

        assert_eq!(
            open_validated_escrow(&mut journal, &mut book, &mut insufficient, request),
            Err(CoordinatorError::FundingCapabilityAmountTooSmall)
        );
        assert_eq!(read_balance(&journal, payer()).unwrap(), 100);
        assert_eq!(read_total_execution_balance(&journal).unwrap(), 100);

        let mut valid = validator(FeeSourceKind::ExecutionBalance, 100);
        assert!(open_validated_escrow(&mut journal, &mut book, &mut valid, request).is_ok());
    }

    #[test]
    fn double_settlement_fails_without_second_refund_or_total_debit() {
        let source = EmptySource;
        let mut journal = journal(&source);
        seed_accounting(&mut journal, 100);
        let mut book = EscrowBookV1::new(4).unwrap();
        let ticket = open_ticket(&mut journal, &mut book, FeeSourceKind::ExecutionBalance);
        let mut effects = effects();
        journal.begin_frame().unwrap();
        effects.begin().unwrap();
        let meter = meter(5);
        let mut terminal = CoordinatorTerminalV1::Running;
        settle_top_level_execution(
            &mut journal,
            &mut effects,
            &mut book,
            ticket,
            &meter,
            &mut terminal,
            RuntimeCallResultV1::Success(Vec::new()),
        )
        .unwrap();
        assert_eq!(read_balance(&journal, payer()).unwrap(), 90);
        assert_eq!(read_total_execution_balance(&journal).unwrap(), 90);

        journal.begin_frame().unwrap();
        effects.begin().unwrap();
        let mut terminal = CoordinatorTerminalV1::Running;
        assert_eq!(
            settle_top_level_execution(
                &mut journal,
                &mut effects,
                &mut book,
                ticket,
                &meter,
                &mut terminal,
                RuntimeCallResultV1::Success(Vec::new()),
            ),
            Err(CoordinatorError::FeeSettlementFailed)
        );
        assert_eq!(read_balance(&journal, payer()).unwrap(), 90);
        assert_eq!(read_total_execution_balance(&journal).unwrap(), 90);
    }

    #[test]
    fn fatal_after_escrow_returns_no_settlement_and_discards_top_level_child() {
        let source = EmptySource;
        let mut journal = journal(&source);
        seed_accounting(&mut journal, 100);
        let mut book = EscrowBookV1::new(4).unwrap();
        let ticket = open_ticket(&mut journal, &mut book, FeeSourceKind::ExecutionBalance);
        let mut effects = effects();
        stage_top_level_child(&mut journal, &mut effects);
        let meter = meter(5);
        let mut terminal = CoordinatorTerminalV1::Fatal;

        assert_eq!(
            settle_top_level_execution(
                &mut journal,
                &mut effects,
                &mut book,
                ticket,
                &meter,
                &mut terminal,
                RuntimeCallResultV1::Success(Vec::new()),
            ),
            Err(CoordinatorError::FatalExecution)
        );
        assert_eq!(read_balance(&journal, payer()).unwrap(), 60);
        assert_eq!(read_total_execution_balance(&journal).unwrap(), 100);
        assert_eq!(read_balance(&journal, target()).unwrap(), 0);
        assert!(effects.root_events().is_empty());
    }
}
