# Oregon — current continuation record

Updated: 2026-09-11 UTC.

## Resume here

- Repository: `zafersari82/Oregon`.
- Accepted `main`: `529b6e8fd50ad4a16d69485b5720b89e979055aa`.
- Stage 4B PR #20 is merged. Its merge tree is `feac8808b0b9549697b72b259e91ae1d40c4deaf`, identical to the verified checkpoint-successor tree.
- Stage 4B integration record: `docs/checkpoints/OREGON_STAGE4B_MAIN_INTEGRATION.md`.
- Stage 4B implementation checkpoint remains: `docs/checkpoints/OREGON_RUNTIME_COORDINATOR_PROGRESS.md`.
- Trust roadmap: `docs/checkpoints/OREGON_TRUST_VERIFICATION_ROADMAP.md`.
- Platform status: `docs/checkpoints/OREGON_PLATFORM_COMPLETION_STATUS.md`.

The old continuation state that said Stage 4B was still isolated on `work/runtime-coordinator-v1-2026-09-06` is superseded by the actual main merge. Do not reopen PR #20 or request Stage 4B main integration again.

## Stage 4B exact integration evidence

- Verified implementation head: `3a71cfb44b46d303d1c0eae87649b6921ad3b348`.
- Verified checkpoint successor: `9b23c642cf733744b4b4b71250f27cc7073f7aad`.
- Actual main merge: `529b6e8fd50ad4a16d69485b5720b89e979055aa`.
- Oregon Rust CI run `34517797355`: SUCCESS on exact main merge.
- Oregon Runtime Coordinator Vectors run `34517797314`: SUCCESS on x86_64 and ARM.
- Oregon Runtime Journal Vectors run `34517797466`: SUCCESS on x86_64 and ARM.
- Oregon Fee Settlement Vectors run `34517797360`: SUCCESS on x86_64 and ARM.
- Oregon Execution Resource Vectors run `34517797377`: SUCCESS on x86_64 and ARM.
- Stage 4B runtime-coordinator mutation gate remains **16/16 compiled mutations killed**.

## Current trust-track state

- Workstream A — Reserve Conservation Proof V1: integrated and bounded-model scope recorded.
- Workstream B — Mutation Evidence Publication V1: integrated and reproducible evidence recorded.
- Workstream C — RandomX public testnet/adversarial challenge: **readiness-gated and not launched**.

The roadmap requires Workstream C to wait until outside participants can use a separately verified node/testnet path. Existing RandomX correctness evidence alone is not launch authorization.

## Immediate next action

1. Reconcile the platform completion/status record with the completed Stage 4B merge and completed Workstreams A/B.
2. Read the Workstream C start gate and public-testnet readiness requirements together with the M6/node exclusions.
3. Propose a **versioned Workstream C readiness design** covering deterministic testnet/genesis isolation, node/miner startup, spend authorization, peer bootstrap/discovery, observability, reproducible incident capture and external-participant setup.
4. Keep public launch, monetary bounties and production/mainnet exposure separately gated.
5. Do not activate Stage 4C, EVM, WASM, universal-envelope block execution or durable execution publication as a side effect of readiness work.

The readiness design is architectural work and must be reviewed before implementation. Stage 4C async execution remains a separate future design/implementation slice.

## Accepted foundations to preserve

- M0–M6 accepted native baseline and networking/node foundations.
- Stage 3B main integration: `fe762f7a5670d94a486423327e3a525cec24afb5`.
- Stage 4A main integration: `dee4ca6ea3b6dc75e4920ab56a44e2b5da8aa0a3`.
- Reserve proof integration: `4670d4ccedd09f02c2a87a13968e74f1fdfa9f4b`.
- Mutation-evidence integration: `cc791386899a777344404b98278cf41f0752f173`.
- Stage 4B main integration: `529b6e8fd50ad4a16d69485b5720b89e979055aa`.

Preserve the existing architecture constraints: Multi-VM direction, hybrid native-UTXO/contract-state model, no fee burn in Execution Architecture V1, exact 1:1 execution-balance backing, downward dependency ownership, fail-closed bounded attacker-controlled resources, and explicit height-based activation without runtime administrator authority.

Always inspect the real branch/PR/CI state when resuming. This HANDOFF is continuation evidence, not a substitute for repository state.
