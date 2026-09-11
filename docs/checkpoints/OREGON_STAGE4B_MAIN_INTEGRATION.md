# Oregon Stage 4B Main Integration Record

Recorded: 2026-09-11 UTC. Stage 4B runtime ABI and transaction coordinator foundation is integrated into `main` as **inactive/test-only infrastructure**.

## Integration

PR #20 was merged after exact implementation-head verification, checkpoint-successor verification and the repository-required owner integration approval.

- PR: https://github.com/zafersari82/Oregon/pull/20
- Previous main: `cc791386899a777344404b98278cf41f0752f173`.
- Verified implementation source: `3a71cfb44b46d303d1c0eae87649b6921ad3b348`.
- Verified implementation tree: `3bf77ee5338f0d8d40104db97eed70802ac9072f`.
- Verified checkpoint successor: `9b23c642cf733744b4b4b71250f27cc7073f7aad`.
- Verified checkpoint tree: `feac8808b0b9549697b72b259e91ae1d40c4deaf`.
- Actual main merge: `529b6e8fd50ad4a16d69485b5720b89e979055aa`.
- Actual merge tree: `feac8808b0b9549697b72b259e91ae1d40c4deaf`.

The actual merge tree is identical to the verified checkpoint-successor tree.

## Pre-merge closure evidence

Implementation head `3a71cfb44b46d303d1c0eae87649b6921ad3b348`:

- Oregon Rust CI run `34510994130`, job `102984780274` — SUCCESS.
- Oregon Runtime Coordinator Vectors run `34510994146` — SUCCESS.
  - x86_64 job `102984780209` — SUCCESS.
  - ARM job `102984780501` — SUCCESS.
- Runtime coordinator mutation gate: **16/16 compiled mutations killed**.
- Full workspace tests, inherited mutation gates, architecture checks, rustdoc/docs, Format and Clippy `-D warnings` — SUCCESS.

Checkpoint successor `9b23c642cf733744b4b4b71250f27cc7073f7aad`:

- Oregon Rust CI run `34512710256`, job `102990519190` — SUCCESS.
- Oregon Runtime Coordinator Vectors run `34512710179` — SUCCESS on x86_64 and ARM.

## Actual main-merge evidence

All runs below target exact main SHA `529b6e8fd50ad4a16d69485b5720b89e979055aa` and completed successfully:

- Oregon Rust CI run `34517797355`, job `103007506080`.
- Oregon Runtime Coordinator Vectors run `34517797314`.
  - x86_64 job `103007506491`.
  - ARM job `103007506893`.
- Oregon Runtime Journal Vectors run `34517797466`.
  - x86_64 job `103007506553`.
  - ARM job `103007506756`.
- Oregon Fee Settlement Vectors run `34517797360`.
  - x86_64 job `103007506896`.
  - ARM job `103007506792`.
- Oregon Execution Resource Vectors run `34517797377`.
  - x86_64 job `103007506273`.
  - ARM job `103007505854`.

The main Rust CI retained the complete inherited mutation suite and the Stage 4B **16/16** runtime-coordinator mutation gate.

## Integrated Stage 4B scope

Stage 4B now makes the following inactive foundations available on `main`:

- VM-neutral `oregon-runtime` ABI types and traits with downward-only dependencies;
- canonical execution events, effects and fixed-width execution receipts;
- deterministic transaction coordinator composition of Stage 3A metering, Stage 3B funding/escrow/settlement and Stage 4A journal/effect rollback;
- chain/height/txid/parent/domain/principal/caller/depth identity binding;
- one shared sticky weight meter across nested execution;
- deterministic nested success/revert/trap/resource-exhaustion handling in test-only backends;
- Phase A / Phase B unpublished proposal composition with receipt separation;
- dedicated x86_64/ARM independent vectors and 16 named mutation controls.

## Non-activation boundary

This integration does **not** activate or deliver:

- a production EVM or WASM backend;
- active universal-envelope execution in blocks;
- execution-state durability/publication in chainstate;
- mempool or RPC execution wiring;
- Stage 4C async delivery/expiry/exactly-once consumption;
- production wallet/spend authorization;
- public testnet configuration, discovery/bootstrap or mining RPC;
- protocol activation or completed Target 2.

The coordinator remains test-gated. Stage 4B integration is foundation availability, not live protocol execution.

## Next continuation

Trust Workstreams A and B are already integrated. Under `docs/checkpoints/OREGON_TRUST_VERIFICATION_ROADMAP.md`, the next trust-track target is **Workstream C readiness**, not public launch itself.

Before any public RandomX testnet/challenge, write and review a versioned readiness design covering at minimum deterministic testnet/genesis isolation, node/miner startup, spend authorization, peer bootstrap/discovery, observability, reproducible incident capture and external-participant setup. Public launch and any monetary bounty remain separately gated.

Stage 4C async execution also remains a separate future architecture/implementation slice and is not implicitly authorized by this integration record.
