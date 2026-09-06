# Oregon — current continuation record

Updated: 2026-09-06 UTC. Progress record only; no activation or main-integration approval.

## Resume here

- Repository: https://github.com/zafersari82/Oregon
- Accepted main: `bf7675bfe17182f77d4c43e2bcbd0c283709d799` (M0–M6 plus platform architecture contract).
- Execution architecture: `ed67ccb89131970571d93911cf5553be33636e2f`, PR #9.
- Verified address checkpoint: `8057ba2a030a8e79c10a240d48675be758c4d875`, PR #10.
- Verified Stage 1 envelope/auth final head: `b97f9d3af9e2c9c4011750cfb69cce8fd9117a8a`, PR #12; byte design PR #11.
- Stage 2 design: `8a3f2c51f7c4ed7078fa808b2223f3b6af4ef3a7`, PR #13.
- **Active continuation branch: `work/contract-state-v1-2026-09-05`, draft PR #14.**
- **Verified Stage 2 implementation: `6bfc3364c71674c2bc3dd20e2b66a4fcabaac19f`.**
- Verified tree: `682a1fc617dad937efbaa3b24ae5c7d861674ad6`.
- Checkpoint: `docs/checkpoints/OREGON_CONTRACT_STATE_PROGRESS.md`.
- Completed plan/spec: `docs/superpowers/{plans,specs}/2026-09-05-contract-state-commitments-v1.md`.

## Completed work

Stage 1 address/envelope/auth primitives and Stage 2 logical contract state/commitments are complete and inactive. Stage 2 includes canonical child/aggregate commitments, immutable SMT transitions/snapshots, checked reads and compressed membership/non-membership proofs.

The continuation audit fixed old-value corruption bypass during deletion/replacement/no-op and cross-decode-domain proof canonicality bypass. Shared rule ownership, resource boundaries, snapshot tests, independent accounting/all-domain vectors and every required mutation target are covered.

Verified implementation evidence:

- Oregon Rust CI `33990488016`, job `101371722660`: SUCCESS, including full workspace, architecture, docs, rustfmt and Clippy.
- Address mutations 3/3, envelope mutations 9/9, state mutations 17/17: killed with clean restoration.
- RandomX architecture `33990488002`: x86 and ARM SUCCESS.
- RandomX full/light parity `33990487993`: x86 and ARM SUCCESS.
- Focused local tests: 39 contract-state + 5 commitment tests passed.

This handoff/checkpoint successor removes the two temporary RandomX trigger entries; code/tests/vectors remain identical to the verified implementation. Always inspect the current branch's own final Rust CI and PR #14 before treating its current head as verified. An earlier green ancestor does not prove changed descendant code.

## Exact next action

Read `AGENTS.md`, the Engineering Constitution, Platform Architecture Contract, Execution Architecture V1, this handoff and the Stage 2 checkpoint. Fetch current branch HEAD before editing.

Start Execution Architecture V1 §27 **Stage 3 with a versioned design** for normalized resource weight, fee escrow/settlement, fee-state transitions and native UTXO reserve conservation. Define authoritative owners, deterministic arithmetic/overflow rules, rollback versus non-revertible fee boundaries, backing invariants, required vectors and adversarial tests before implementation. Preserve no fee burn, one fee/execution truth and 1:1 native backing. Activation constants require their specified benchmark/vector evidence.

Do not repeat M0–M6, Stage 1, Stage 2 design selection or completed Stage 2 implementation. The owner requests autonomous progress without repeated low-level approval prompts; that does not authorize changing frozen architecture or integrating main.

## Remaining sequence and boundaries

Later work is Stage 3 accounting/weight/fees, runtime/journal/async core, EVM ingress/backend, WASM backend, cross-VM operations, unified mempool/block execution, durable chainstate/reorg/recovery, then separate activation and integration decisions.

Current header/transaction bytes, RocksDB schema, UTXO and monetary rules, active M0–M6 behavior and main remain unchanged. No wallet/RPC, public network or active contracts are claimed. Never force-push accepted/shared checkpoint refs. Keep exact sources, verification evidence, limitations and next action in the repository.
