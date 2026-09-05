# Oregon — current continuation record

Updated: 2026-09-05 (UTC). This file records development progress; it is not an activation or main-integration approval.

## Where to resume

- Repository: https://github.com/zafersari82/Oregon
- Accepted main for this execution work: `bf7675bfe17182f77d4c43e2bcbd0c283709d799` (M0–M6 implementation plus platform architecture contract).
- Owner-approved execution architecture: `design/execution-architecture-v1-2026-09-05`, source commit `ed67ccb89131970571d93911cf5553be33636e2f`, PR #9.
- Verified address checkpoint/source: `8057ba2a030a8e79c10a240d48675be758c4d875`, branch `work/execution-addresses-2026-09-05`, PR #10.
- Byte-exact envelope wire design: `design/execution-envelope-wire-v1-2026-09-05`, source commit `dd473f0277df1f82d51390332a5473a708031be0`, PR #11.
- **Active continuation branch:** `work/execution-envelope-auth-v1-2026-09-05`, PR #12.
- Wire spec: `docs/superpowers/specs/2026-09-05-execution-envelope-wire-v1.md`.
- Completed implementation plan: `docs/superpowers/plans/2026-09-05-execution-envelope-wire-v1.md`.
- Address checkpoint: `docs/checkpoints/OREGON_EXECUTION_ADDRESS_PROGRESS.md`.
- Envelope/auth checkpoint: `docs/checkpoints/OREGON_EXECUTION_ENVELOPE_PROGRESS.md`.

## Current verified implementation source

Inactive universal-envelope and authorization outer-wire code is complete at implementation source commit `df62a75dfbde80dbea72e599ffbbccf0fb8fe1e0`, tree `47faba5451bc929804137f33de77b3973dabb0d2`.

Authoritative GitHub Actions verification for that code source:

- Oregon Rust CI run `33980111067`, job `101343713813`: SUCCESS.
- Architecture scan: SUCCESS.
- Execution address contracts: SUCCESS.
- Execution envelope contracts: SUCCESS.
- Full workspace/all-target tests: SUCCESS.
- Inherited execution-address mutations: 3/3 killed.
- Execution-envelope mutations: 9/9 killed.
- Chainstate rustdoc: SUCCESS.
- Workspace docs: SUCCESS.
- `cargo fmt --check`: SUCCESS.
- Clippy with `-D warnings`: SUCCESS.

The envelope implementation remains **inactive**. It does not route through current `Transaction::encode/decode/txid`, blocks, mempool, chainstate, RPC, wallet, EVM, WASM or native UTXO execution.

## First action in another conversation

Fetch the current head of `work/execution-envelope-auth-v1-2026-09-05` (or a later successor branch named by this file), then read this handoff, `AGENTS.md`, the Engineering Constitution, Platform Architecture Contract, Execution Architecture V1, the address checkpoint and the envelope checkpoint before editing.

Do not repeat M0–M6, typed-address work, or envelope/auth outer-wire work. Do not restart their architecture selection. Preserve the repository's versioned design/test/checkpoint process.

## Verification environment

GitHub Actions is authoritative for Rust verification. Every code-bearing descendant must own its own exact-head verification before being called verified. A green ancestor does not prove changed descendant code.

## Remaining execution sequence

Execution Architecture V1 §27 remains authoritative:

1. Inactive execution primitives: **typed address complete; universal envelope + authorization outer wire complete.**
2. **Logical contract state and versioned commitments — next.**
3. Resource weight, fee escrow/state transition and UTXO reserve conservation.
4. Deterministic runtime, call journal and asynchronous message core.
5. EVM backend and Ethereum normalization/ingress.
6. Deterministic WASM backend.
7. Cross-VM calls and execution balance transfers.
8. Unified mempool and block execution.
9. Durable chainstate/reorg/recovery integration, complete vectors and mutations.
10. Separate activation/checkpoint and main integration decisions.

### Exact next action

Start Stage 2 by writing a versioned design for **logical contract state and versioned commitments** before implementation. The design must freeze ownership and exact commitment boundaries for EVM state, WASM state, execution reserve/accounting state, asynchronous-message state, and the extensible header commitment without changing current accepted block/header bytes prematurely.

The Stage 2 design must explicitly cover deterministic state-root algorithms/interfaces, domain separation, empty-state roots, versioning/upgrades, proof/commitment boundaries, persistence ownership, reorg/recovery behavior, resource bounds, and how future EVM/WASM backends consume the same authoritative committed state. Do not infer unspecified consensus bytes from Ethereum merely for compatibility.

## Persistence and authority

Keep completed work, exact source SHAs/trees, verification evidence, limitations and next action in this file and checkpoints. Never force-push accepted/shared checkpoint refs and never integrate `main` without the separate integration decision required by the repository.
