# Oregon — current continuation record

Updated: 2026-09-10 UTC.

## Resume here

- Repository: `zafersari82/Oregon`.
- Active branch: `work/runtime-coordinator-v1-2026-09-06`.
- Draft PR: #20, `Stage 4B: runtime ABI and transaction coordinator`.
- Main baseline remains `cc791386899a777344404b98278cf41f0752f173`; Stage 4B is not integrated.
- Exact verified Stage 4B implementation head: `3a71cfb44b46d303d1c0eae87649b6921ad3b348`.
- Exact implementation tree: `3bf77ee5338f0d8d40104db97eed70802ac9072f`.
- Checkpoint: `docs/checkpoints/OREGON_RUNTIME_COORDINATOR_PROGRESS.md`.
- Plan: `docs/superpowers/plans/2026-09-06-runtime-coordinator-v1.md`.
- Spec: `docs/superpowers/specs/2026-09-06-runtime-coordinator-v1-design.md`.

Stage 4B Tasks 1–10 are implemented and exact implementation-head verification is complete. Task 11 implementation-head verification and source review are complete; the checkpoint commit containing this HANDOFF must now be verified as the successor exact head before PR evidence is finalized.

Exact implementation-head evidence:

- Oregon Rust CI run `34510994130`, job `102984780274`: **SUCCESS**. It checked PR synthetic merge `9494e08cb6bdee1999b304f151471e96329014af`, explicitly composed from exact branch head `3a71cfb44b46d303d1c0eae87649b6921ad3b348` over unchanged main `cc791386899a777344404b98278cf41f0752f173`.
- Oregon Runtime Coordinator Vectors run `34510994146`: **SUCCESS**.
  - x86_64 `ubuntu-24.04` job `102984780209`: **SUCCESS**.
  - ARM `ubuntu-24.04-arm` job `102984780501`: **SUCCESS**.
- Runtime coordinator mutation gate: **16/16 compiled mutations killed**. Compiler failure is not accepted as a killed mutant.
- Full workspace tests, inherited mutation gates, docs, Format and Clippy `-D warnings` are green on the exact implementation evidence.

Fresh source review found no P1/P2 blocker. Dependency direction remains correct, the coordinator and adversarial backends remain test-only, VM-visible state access remains WASM-only, EVM effects require `EvmCommitmentV1`, Stage 3B remains fee arithmetic authority, the shared meter has no reset/refund path, and Phase A cannot escape after Phase-B failure.

## Immediate next action

1. Verify the checkpoint successor commit itself with Oregon Rust CI.
2. Verify the same successor with `Oregon Runtime Coordinator Vectors` on both x86_64 and ARM.
3. Update PR #20 body with the implementation SHA/tree, implementation CI, 16/16 mutation evidence, checkpoint-successor SHA/tree and successor CI run/job IDs.
4. Do not create another repository commit solely to embed successor run IDs.
5. Stop before `main` integration. PR #20 stays isolated/draft unless the owner later makes a separate explicit integration decision.

## Stage 4B claim boundary

The coordinator is still included through `#[cfg(test)]`; this slice does not activate production WASM/EVM execution. There is no Stage 4B chainstate persistence/publication, mempool/RPC wiring, live protocol execution path or main integration. Do not call Stage 4B production activated or completed Target 2.

The owning coordinator path does compose validated funding/escrow, runtime dispatch, settlement, finalized Phase A, surviving effects and Phase B in deterministic tests. Identity binding covers chain, height, txid, parent hash, execution domain, principal, caller and top-level depth. Escrow reservation is not child-spendable; deterministic revert/trap rolls back execution effects but not shared-meter consumption; fatal/resource exhaustion cannot be converted into success.

## Accepted foundations to preserve

- Stage 3B main integration: `fe762f7a5670d94a486423327e3a525cec24afb5`.
- Stage 4A main integration: `dee4ca6ea3b6dc75e4920ab56a44e2b5da8aa0a3`.
- Reserve proof integration: `4670d4ccedd09f02c2a87a13968e74f1fdfa9f4b`.
- Mutation-evidence publication/main baseline: `cc791386899a777344404b98278cf41f0752f173`.

Reserve Conservation Proof V1 remains accepted at source SHA `511f61302ee486e618a33235808572ae4419f487`, tree `e2f12792a08a5376807baa4e92b4ca63ef44a56b`. Its Kani proof scope remains bounded-model proof plus differential correspondence tests, not proof of arbitrary production state/storage/VM behavior.

Preserve the existing cautions: `FundingCapabilityV1::new` validates data but is not by itself live source authority; `WeightMeter` exhaustion is sticky; generic unrestricted VM-facing system-domain writes are forbidden; accepted historical refs must not be force-moved.

Always inspect the actual branch, PR and exact-head CI when resuming. This HANDOFF is continuation evidence, not a substitute for repository state.