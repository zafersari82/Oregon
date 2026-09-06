# Oregon Stage 4A — bounded runtime journal checkpoint

Updated: 2026-09-06 UTC.

## Status

Stage 4A implementation is **verified on its exact implementation head** and remains inactive on draft PR #19. It is not integrated into `main`, not persisted by chainstate/storage, and not an execution/VM activation.

- Base `main`: `fe762f7a5670d94a486423327e3a525cec24afb5`
- Implementation branch: `work/runtime-journal-v1-2026-09-06`
- Draft PR: #19 — `Stage 4A: bounded execution journal foundation`
- Verified implementation SHA: `f15ac4db43801fed3dad3d9fb03268e7d34cf43a`
- Verified implementation tree: `24d6c311b005a47cc146a7f46f6d7d319996cac5`

A separate owner integration decision is still required before any merge to `main`.

## Approved scope delivered

Stage 4A adds an unpublished, bounded execution journal in `oregon-execution` over immutable Stage 2 Oregon SMT snapshots. It depends downward on `oregon-contract-state`; no VM, runtime coordinator, storage, chainstate, node or network dependency was added.

The public Stage 4A surface includes:

- `JournalContextV1`
- `JournalLimitsV1`
- `ExecutionJournalV1`
- `JournalIntentV1`
- `JournalResultV1`
- typed `JournalError`

The implementation preserves the approved design:

- only `Wasm`, `ExecutionAccounting`, `ExecutionReceipts`, `AsyncOutbox`, `AsyncConsumed` and `FeeState` may be journaled;
- `NativeUtxo` and `Evm` are rejected;
- one root frame, bounded child frames, top-frame commit/revert and ancestor-revert semantics;
- delete shadows lower layers while a present empty value remains distinct from absence;
- checked Stage 2 reads remain authoritative below the overlay;
- limits are checked before retained caller bytes are inserted;
- live depth, total-created frame count, live entry count and retained raw key/value bytes remain independently bounded;
- child commit ownership-moves entries into the parent and remains revertible by ancestors;
- finalization consumes the journal, rejects open children and returns no partial multi-domain prefix;
- committed finalization returns immutable `StateTransition` proposals only after all participating domains succeed;
- reverted finalization returns unchanged roots and no transitions;
- source state and active roots are never mutated by the journal.

Stage 4A does **not** add Stage 4B transaction/fee/receipt composition, Stage 4C async delivery semantics, VM execution, RPC/wire activation, durable execution publication, block admission changes or mainnet/testnet activation.

## Test-first evidence

The expected RED boundary was preserved before production API export. Public CI observed the new journal tests fail on unresolved journal API imports before the implementation was exposed; implementation then proceeded without weakening those tests.

Focused coverage now includes nested commit/revert, sibling isolation, delete versus present-empty, equivalent legal traces, constructor/domain rejection, root lifecycle misuse, exact/one-over structural and byte/entry bounds, frame-count non-refund, rejection atomicity, corrupt old-value handling, late-domain failure with no partial bundle, committed/reverted unpublished results, checked base reads and a bounded independent clone-map property model.

## Exact-head verification evidence

Verified implementation head: `f15ac4db43801fed3dad3d9fb03268e7d34cf43a`.

### Full Rust CI

- Workflow: `Oregon Rust CI` #769
- Run ID: `34046504969`
- Job ID: `101522383985`
- Conclusion: `SUCCESS`

The exact-head job passed:

- architecture scan;
- execution address contracts;
- execution envelope contracts;
- state commitment contracts;
- contract state contracts;
- execution resource contracts;
- fee settlement contracts;
- runtime journal contracts;
- full workspace/all-target tests;
- execution address mutation gates: **3/3 killed**;
- execution envelope mutation gates: **9/9 killed**;
- contract state mutation gates: **17/17 killed**;
- execution resource mutation gates: **13/13 killed**;
- fee settlement mutation gates: **14/14 killed**;
- runtime journal mutation gates: **12/12 killed**;
- chainstate rustdoc;
- workspace docs;
- `cargo fmt --check`;
- warnings-denied workspace/all-target Clippy.

### Independent Stage 4A vectors

- Workflow: `Oregon Runtime Journal Vectors` #22
- Run ID: `34046505066`
- x86_64 job: `101522384222` — `SUCCESS`
- ARM job: `101522384312` — `SUCCESS`

Both jobs verified the committed independent Python oracle and bounded Rust journal traces. The corpus contains six named cases. The journal mutation gate also rejects a deliberately changed expected root before restoring the committed corpus, so the oracle/consumer path has a negative control rather than only a happy-path comparison.

## Environment note

The working container could not reliably resolve GitHub for a local clone, so no local full-workspace result is claimed. Exact-head GitHub Actions is the authoritative verification evidence above. This limitation did not cause any test, lint or mutation gate to be skipped in CI.

## Inactive boundaries

- No current block/transaction bytes are changed.
- No Stage 4A result is durable or consensus-published.
- No VM-facing unrestricted system-domain write API exists.
- EVM state remains outside the Oregon SMT journal.
- Existing fee/meter semantics remain unchanged; rollback does not reset consumed weight or cumulative conversion accounting.
- `main` remains `fe762f7a5670d94a486423327e3a525cec24afb5` until a separate integration decision.

## Next action

This checkpoint and `HANDOFF.md` are the documentation successor to the verified implementation head. After this checkpoint commit is created, verify that successor's own exact-head CI and vector workflows, update PR #19 with closure evidence, then stop before `main` integration for the separate owner decision.
