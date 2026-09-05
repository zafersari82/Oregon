# Oregon — current continuation record

Updated: 2026-09-05 (UTC). This file records development progress; it is not an activation or main-integration approval.

## Where to resume

- Repository: https://github.com/zafersari82/Oregon
- Accepted main for this execution work: `bf7675bfe17182f77d4c43e2bcbd0c283709d799` (M0–M6 implementation plus platform architecture contract).
- Owner-approved execution architecture: `design/execution-architecture-v1-2026-09-05`, source commit `ed67ccb89131970571d93911cf5553be33636e2f`, PR #9.
- Verified address checkpoint/source for current work: `8057ba2a030a8e79c10a240d48675be758c4d875`, branch `work/execution-addresses-2026-09-05`, PR #10.
- **Active continuation branch:** `design/execution-envelope-wire-v1-2026-09-05`, created directly from the verified address checkpoint.
- Current byte-exact spec: `docs/superpowers/specs/2026-09-05-execution-envelope-wire-v1.md`.
- Current implementation plan: `docs/superpowers/plans/2026-09-05-execution-envelope-wire-v1.md`.
- Address progress checkpoint: `docs/checkpoints/OREGON_EXECUTION_ADDRESS_PROGRESS.md`.
- **Current status:** typed execution addresses are complete/verified. Universal envelope + authorization outer-wire decisions are being recorded from the already approved execution architecture before inactive implementation. No activation path has changed.

## First action in another conversation

Fetch current refs and inspect the latest descendant of `design/execution-envelope-wire-v1-2026-09-05` or a later successor branch named by this file. Read this handoff, `AGENTS.md`, the Engineering Constitution, Platform Architecture Contract, Execution Architecture V1, the execution-address checkpoint, the envelope wire spec and the current plan before editing.

Do not repeat accepted M6 work or completed address work. Do not restart architecture selection. The owner has already delegated Oregon technical implementation choices and the execution architecture is frozen; continue the first incomplete plan step and preserve the repository's versioned/checkpoint process.

## Verification environment

GitHub Actions is the authoritative Rust verification environment for this continuation. Every code-bearing descendant commit must record its own exact red/green/full-CI/mutation evidence before being called verified. A previous green ancestor does not prove a changed descendant.

## Remaining execution sequence

Execution Architecture V1 §27 remains authoritative:

1. Inactive execution primitives: typed address complete; envelope/authorization outer wire is current work.
2. Logical contract state and versioned commitments.
3. Resource weight, fee escrow/state transition and UTXO reserve conservation.
4. Deterministic runtime, call journal and asynchronous message core.
5. EVM backend and Ethereum normalization/ingress.
6. Deterministic WASM backend.
7. Cross-VM calls and execution balance transfers.
8. Unified mempool and block execution.
9. Durable chainstate/reorg/recovery integration, complete vectors and mutations.
10. Separate activation/checkpoint and main integration decisions.

### Exact next action

Commit and verify the byte-exact envelope/authorization wire specification and its implementation plan on the active design branch. Then create a successor implementation branch, publish test-only vectors/contracts first to obtain the expected-red CI, implement the inactive `oregon-primitives` envelope/auth outer wire, run the required targeted mutations and full inherited gates, and write a progress checkpoint.

The implementation must not modify or route through current `Transaction::encode/decode/txid`, blocks, mempool, chainstate, RPC, wallet, EVM or WASM execution.

## Persistence and authority

Keep completed work, exact SHAs/trees, test evidence, limitations and next action in this file and checkpoints. Publish coherent increments and verify the remote ref before reporting them saved. Never force-push accepted/shared checkpoint refs and never integrate `main` without the separate integration decision required by the repository.
