# Oregon Runtime Coordinator V1 Progress Checkpoint

Recorded: 2026-09-10 UTC.

This checkpoint records the verified inactive Stage 4B runtime/coordinator implementation. It is continuation evidence only; it does not activate a production VM, publish execution state, integrate chainstate/mempool/RPC, or merge Stage 4B to `main`.

## Authority and exact identity

- Repository: `zafersari82/Oregon`.
- Isolated branch: `work/runtime-coordinator-v1-2026-09-06`.
- Draft PR: #20, `Stage 4B: runtime ABI and transaction coordinator`.
- Base `main`: `cc791386899a777344404b98278cf41f0752f173`.
- Design: `docs/superpowers/specs/2026-09-06-runtime-coordinator-v1-design.md`.
- Plan: `docs/superpowers/plans/2026-09-06-runtime-coordinator-v1.md`.
- Exact implementation head: `3a71cfb44b46d303d1c0eae87649b6921ad3b348`.
- Exact implementation tree: `3bf77ee5338f0d8d40104db97eed70802ac9072f`.
- PR verification synthetic merge: `9494e08cb6bdee1999b304f151471e96329014af`, consisting of exact implementation head `3a71cfb44b46d303d1c0eae87649b6921ad3b348` over unchanged base `cc791386899a777344404b98278cf41f0752f173`.

The current local/container environment was not used as an acceptance authority for Rust execution. GitHub Actions exact-head evidence below is authoritative for this checkpoint.

## Delivered Stage 4B surface

- `oregon-runtime` remains ABI-only and downward-only.
- The transaction coordinator remains private and `#[cfg(test)]` gated in `oregon-execution`; there is no production coordinator activation in this slice.
- One shared Stage 3A `WeightMeter` covers nested runtime work; rollback does not refund meter consumption and exhaustion is sticky.
- Stage 3B validated funding/escrow and settlement authority is composed with Stage 4A journal/effect rollback rather than reimplemented.
- Funding chain/height/txid and runtime principal/caller/target/domain/depth context are bound before spending authority is consumed.
- Execution-funded escrow is reserved before the execution child and cannot be spent by that child.
- WASM state host access is scoped to Oregon SMT logical state. EVM-labelled test backends receive deterministic state denial and cannot masquerade as Oregon SMT state.
- Nested success/revert/trap, attached value, events, return data, read-only propagation and fatal/resource-exhaustion behavior are covered by test-only deterministic backends.
- Phase A excludes `ExecutionReceipts`; Phase B writes canonical fee and execution receipts only after successful proposal composition.
- `ExecutionReceiptV1` remains exactly 259 bytes and is checked against independent vectors.
- Phase-B failure returns no transaction proposal and does not consume the caller's external escrow book.

## Exact implementation-head verification

### Oregon Rust CI

Run `34510994130`, job `102984780274`: **SUCCESS**.

This pull-request run checked synthetic merge `9494e08cb6bdee1999b304f151471e96329014af`, whose checkout log explicitly identifies exact branch head `3a71cfb44b46d303d1c0eae87649b6921ad3b348` merged over unchanged main `cc791386899a777344404b98278cf41f0752f173`.

Successful gates include architecture scans, execution address/envelope/state contracts, contract-state tests, Stage 3A resource tests, Stage 3B fee-settlement tests, Stage 4A journal tests, Stage 4B runtime coordinator contracts, full workspace `--all-targets`, every inherited mutation gate, the Stage 4B 16-mutation gate, rustdoc, workspace docs, Format and Clippy with `-D warnings`.

The exact-head execution suite included 74 passing `oregon-execution` tests with zero failures. Focused coordinator execution included the owning-path, Phase-B failure, domain/identity binding, escrow visibility, EVM state denial, nested WASM/EVM-labelled/WASM trace, rollback, fatal propagation and resource-exhaustion cases.

### Dedicated Runtime Coordinator x86_64 / ARM

Run `34510994146`: **SUCCESS**.

- `ubuntu-24.04` job `102984780209`: **SUCCESS**.
- `ubuntu-24.04-arm` job `102984780501`: **SUCCESS**.

Both jobs passed Stage 4B architecture/dependency checks, the independent Python runtime coordinator oracle, runtime receipt primitives, `oregon-runtime --all-targets`, two consecutive focused coordinator passes and `oregon-execution --all-targets`.

## Runtime coordinator mutation evidence

The exact implementation-head Rust CI reports **16/16 compiled mutations killed**. The runner rejects compilation failure as mutation success and restores/checks source after each mutation.

1. `rollback_refunds_meter`
2. `backend_chooses_cheaper_resource_domain`
3. `read_only_child_clears_parent_restriction`
4. `event_survives_reverted_frame`
5. `attached_value_survives_reverted_frame`
6. `escrow_reservation_remains_spendable`
7. `execution_charge_does_not_reduce_execution_total`
8. `trap_maps_to_committed_fee_outcome`
9. `resource_exhaustion_masked_by_backend_success`
10. `generic_system_domain_state_write_exposed`
11. `evm_effect_accepted_as_oregon_smt`
12. `receipt_state_root_inserted_into_phase_a`
13. `phase_a_result_escapes_after_phase_b_failure`
14. `duplicate_receipt_finalization_succeeds`
15. `non_trapped_receipt_accepts_nonzero_trap_code`
16. `event_return_overflow_truncates`

The independent-vector negative control also changed a runtime receipt byte and was rejected by both the Python oracle and Rust consumer before the clean corpus was restored.

Inherited exact-head mutation evidence also remained green: execution addresses 3/3, execution envelopes 9/9, contract state 17/17, execution resources 13/13, fee settlement 14/14 and runtime journal 12/12.

## Fresh source review

A fresh review on implementation head `3a71cfb44b46d303d1c0eae87649b6921ad3b348` found no P1/P2 blocker before checkpointing:

- coordinator remains test-gated; no production backend or coordinator export was introduced;
- `oregon-runtime` depends only downward on primitives/`thiserror` and has no state/execution/VM/persistence/network dependency;
- `oregon-execution` has no storage, chainstate, RPC or production VM implementation dependency;
- VM-visible state host support is WASM-only; EVM-labelled calls receive deterministic state denial;
- EVM state effects require `EvmCommitmentV1`; Oregon SMT cannot be used to label an EVM effect;
- Stage 3B remains the fee-settlement arithmetic authority; Stage 4B consumes the authoritative settlement result instead of duplicating fee pricing;
- `WeightMeter` exposes no reset/refund path and exhaustion stays sticky at `max_weight`;
- Phase A cannot escape as a publishable proposal after Phase-B failure;
- changed Stage 4B source contains no unfinished `TODO`, `FIXME` or `unimplemented!` production path and no broad lint-suppression workaround.

## Claim boundary and next gate

This checkpoint does **not** claim production WASM/EVM execution, durable execution-state persistence, mempool/RPC/protocol activation, completed Target 2, or `main` integration.

The checkpoint commit containing this file must itself be verified as the successor exact head by Oregon Rust CI and the dedicated x86_64/ARM runtime-coordinator workflow. After those successor runs succeed, their IDs belong in PR #20's body; no extra repository commit should be created solely to record those external run IDs.

After successor verification and PR evidence update, stop before `main` integration. Merging Stage 4B remains a separate explicit owner decision.