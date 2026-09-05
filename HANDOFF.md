# Oregon — current continuation record

Updated: 2026-09-05 (UTC). This file records development progress; it is not an activation or main-integration approval.

## Where to resume

- Repository: https://github.com/zafersari82/Oregon
- Accepted main for this execution work: `bf7675bfe17182f77d4c43e2bcbd0c283709d799` (M0–M6 implementation plus platform architecture contract).
- Latest approved execution design: `design/execution-architecture-v1-2026-09-05`, source commit `ed67ccb89131970571d93911cf5553be33636e2f`, PR #9. This design is newer than main and remains a separate PR.
- **Active continuation branch:** `work/execution-addresses-2026-09-05`, based on that design, not older main.
- Work PR: https://github.com/zafersari82/Oregon/pull/10 (draft, depends on design PR #9).
- Address plan: `docs/superpowers/plans/2026-09-05-execution-addresses.md`.
- Address progress checkpoint: `docs/checkpoints/OREGON_EXECUTION_ADDRESS_PROGRESS.md`.
- **Current status:** the inactive 33-byte typed execution-address foundation is implemented and verified. Do not repeat it.
- Verified implementation/hardening head: `52acaa1b645577314407372b4933d11eef5500cd`.
- Implementation commit: `445a0a92fd22409dd171bb915c2310203534ccca`.
- Test-first evidence: commit `e7bae512cbf47f17b50d9db85c12dbca3ab874da`, Rust CI run `33970929497`, job `101319185976`, failed exactly with E0432 because the `execution_address` module did not exist. This expected red result precedes implementation.
- Green verification for `52acaa1b645577314407372b4933d11eef5500cd`: Oregon Rust CI run `33971365738` SUCCESS; RandomX Architecture Vector run `33971365737` SUCCESS; RandomX Full Light Parity run `33971365761` SUCCESS.
- The Rust CI green run includes architecture scan, focused execution-address contracts, full workspace tests, 3/3 execution-address mutation kills, chainstate rustdoc, workspace docs, rustfmt and workspace Clippy.

## First action in another conversation

Fetch the remote refs, check out the active continuation branch, and reread this file from its latest remote head. Inspect `git status` and the latest commits before editing. Read `AGENTS.md`, both architecture contracts, Execution Architecture V1, the relevant accepted checkpoint, the address progress checkpoint and the current plan. Never assume main contains unmerged design/work branches.

Continue the first incomplete execution stage. Do not repeat accepted M6 work or the completed address primitive, and do not begin from old ZIPs or earlier mutation branches. A later dated handoff on a descendant commit supersedes this entry.

## Current verification environment

The session that completed the address implementation had no local Cargo/Rust toolchain. GitHub Actions supplied the authoritative red/green, workspace and mutation evidence listed above. GitHub writes use the connected repository capability. For any descendant commit, verify the exact remote head and its own required CI before calling that head clean.

## Remaining execution work

The architecture's authoritative sequence is Execution Architecture V1 §27:

1. Inactive execution primitives: **typed address portion complete and verified**; universal envelope and authorization descriptors are still open.
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

Write the detailed **bounded inactive universal-envelope and authorization-descriptor wire specification before implementing either type**. Execution Architecture V1 gives the logical fields and authorization model but intentionally does not freeze every width, discriminant, canonical optional-field/order encoding, hard count/byte limit, signing commitment, source-byte commitment or malformed-input rule.

Do not guess those consensus-facing details from Ethereum defaults. The next design must preserve the existing architecture: one canonical Oregon execution truth, separate principal/fee payer, bounded typed authorization proofs, Oregon domain-separated native commitments, exact signed Ethereum source-byte/hash commitment during normalization, layered replay protection, and no change to accepted M0–M6 bytes until explicit activation.

No envelope or authorization code has been implemented yet.

## Persistence and authority

Keep completed work, test evidence, failures/limitations and the exact next action in this file and linked progress records. Publish coherent commits to the active branch and verify the remote ref/tree before saying work is saved. Link the branch/PR in the conversation. Never move accepted checkpoint refs, force-push shared work, or integrate main without the separate integration decision required by the repository.

No EVM/WASM execution, fee change, envelope activation, RPC, wallet or network launch has been delivered by this address-only task.
