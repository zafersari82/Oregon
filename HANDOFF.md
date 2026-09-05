# Oregon — current continuation record

Updated: 2026-09-05 (UTC). Progress record only; no activation or main-integration approval.

## Where to resume

- Repository: https://github.com/zafersari82/Oregon
- Accepted main: `bf7675bfe17182f77d4c43e2bcbd0c283709d799` (M0–M6 plus platform architecture contract).
- Execution architecture: `ed67ccb89131970571d93911cf5553be33636e2f`, PR #9.
- Verified typed-address checkpoint: `8057ba2a030a8e79c10a240d48675be758c4d875`, PR #10.
- Verified inactive envelope/auth final head: `b97f9d3af9e2c9c4011750cfb69cce8fd9117a8a`, PR #12; byte design PR #11.
- Stage 2 design: `8a3f2c51f7c4ed7078fa808b2223f3b6af4ef3a7`, PR #13.
- **Active continuation branch: `work/contract-state-v1-2026-09-05`, PR #14.**
- Current spec: `docs/superpowers/specs/2026-09-05-contract-state-commitments-v1.md`.
- Current plan: `docs/superpowers/plans/2026-09-05-contract-state-commitments-v1.md`.

## Current work

Stage 1 is complete. Stage 2 implementation includes primitive child/aggregate commitments, the immutable SMT transition engine, checked storage-neutral reads, and compressed membership/non-membership proofs. Stage 2 final acceptance remains incomplete.

Inherited implementation head `d3e89ab13bd70604b14b82893519f54664f594ba` was audited on continuation. Rust CI run `33985620545`, job `101358465281`, passed focused tests, workspace tests, inherited mutations, state mutations and docs, but failed rustfmt; Clippy was skipped. It is not a verified final checkpoint.

The audit identified two correctness bugs being repaired with test-first evidence:

1. Deletion/replacement/same-value writes must validate the old referenced value blob before returning a transition, including a no-op.
2. Proof verification must reject explicit default siblings for the verification domain even if the object was decoded under another domain.

Required remaining work also includes five missing normative mutation targets, exact resource-boundary/adversarial tests, accounting transition/long-prefix vectors, all-id descriptor vectors, snapshot retention coverage and one authoritative path-bit implementation.

## First action in another conversation

Fetch the active branch above, then read this file, `AGENTS.md`, the Engineering Constitution, Platform Architecture Contract, Execution Architecture V1, Stage 2 spec/plan and relevant checkpoints. Inspect exact branch HEAD and CI before editing; a green ancestor does not verify a changed descendant.

Resume Stage 2 hardening and final verification. Do not repeat M0–M6, address, envelope or Stage 2 design selection. The owner delegated remaining Stage 2 technical choices and instructed autonomous progress without repeated approval prompts; frozen architecture and separate main-integration rules remain in force.

## Verification and remaining sequence

Local Rust 1.85.0 focused tests can establish red/green regressions. GitHub Actions provides authoritative full-workspace, architecture, docs, format, Clippy, mutation and x86/ARM RandomX verification for the exact published code state.

After Stage 2 is fully verified, create `docs/checkpoints/OREGON_CONTRACT_STATE_PROGRESS.md`, complete the plan and update this handoff. The next work is a versioned Stage 3 design for normalized resource weight, fee escrow/state transition and UTXO reserve conservation; do not infer activation constants from another chain.

Later stages remain runtime/journal/async core, EVM ingress/backend, WASM backend, cross-VM operations, unified mempool/block execution, durable chainstate/reorg/recovery integration, then separate activation and main-integration decisions.

## Persistence and authority

Preserve exact source SHAs/trees, verification evidence, limitations and next action in checkpoints and this file. Never force-push accepted/shared checkpoint refs. No current header/transaction bytes, storage schema, native UTXO rules or active M0–M6 path is changed by Stage 2.
