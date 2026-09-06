# Oregon — current continuation record

Updated: 2026-09-06 UTC. Stage 3B is integrated into `main`. Stage 4A implementation is exact-head verified and checkpointed on an isolated branch, but is **not integrated or activated**. A post-Stage-4A trust/verification roadmap remains queued in-repo.

## Resume here

- Repository: `zafersari82/Oregon`.
- Current `main`: `fe762f7a5670d94a486423327e3a525cec24afb5`.
- Main tree at Stage 3B integration: `cebbc2370e094052872570069c0a149a4581eb99`.
- Active Stage 4A branch: `work/runtime-journal-v1-2026-09-06`.
- Active draft PR: #19 — `Stage 4A: bounded execution journal foundation`.
- Verified Stage 4A implementation SHA: `f15ac4db43801fed3dad3d9fb03268e7d34cf43a`.
- Verified Stage 4A implementation tree: `24d6c311b005a47cc146a7f46f6d7d319996cac5`.
- Stage 4 design: `docs/superpowers/specs/2026-09-06-runtime-journal-async-v1-design.md`.
- Stage 4A plan: `docs/superpowers/plans/2026-09-06-runtime-journal-v1.md`.
- Stage 4A checkpoint: `docs/checkpoints/OREGON_RUNTIME_JOURNAL_PROGRESS.md`.
- Post-Stage-4A trust roadmap: `docs/checkpoints/OREGON_TRUST_VERIFICATION_ROADMAP.md`.
- Stage 3B integration evidence: `docs/checkpoints/OREGON_STAGE3B_MAIN_INTEGRATION.md`.

Read `AGENTS.md` and all applicable normative architecture/design documents before editing. Inspect the actual current `main`, active branch head, PR and exact-head CI; this record cannot override repository state.

## Integrated foundation

M0–M6, execution addresses, universal-envelope/auth structure, logical contract state, Stage 3A resource metering/base fees and Stage 3B escrow/settlement/reserve foundations are integrated. Stage 3B integration is not execution activation.

## Stage 4A verified implementation

Stage 4 remains decomposed into independently verified slices:

1. **4A:** bounded frame journal over existing Oregon SMT snapshots, nested commit/revert, checked reads and atomic unpublished finalization.
2. **4B:** deterministic runtime/call ABI and transaction/fee/receipt composition, retaining one shared meter and authoritative source validation.
3. **4C:** bounded async message/outbox/consumption lifecycle; exact wire/expiry/delivery semantics require their own detailed design before implementation.

Stage 4A now contains the approved bounded journal surface in `oregon-execution`, with constructor/domain validation, checked overlay reads/writes, child frame lifecycle, ownership-moving child commit, ancestor-revert behavior, exact resource ceilings, atomic unpublished multi-domain finalization, independent vectors and mutation gates.

The verified implementation head is `f15ac4db43801fed3dad3d9fb03268e7d34cf43a`.

Exact-head evidence:

- `Oregon Rust CI` #769 — run `34046504969`, job `101522383985` — `SUCCESS`.
- Full workspace/all-target tests — `SUCCESS`.
- Mutation gates: address **3/3**, envelope **9/9**, contract-state **17/17**, resource **13/13**, fee-settlement **14/14**, journal **12/12**.
- Rustdoc/docs, Format and warnings-denied Clippy — `SUCCESS`.
- `Oregon Runtime Journal Vectors` #22 — run `34046505066`.
- x86_64 job `101522384222` — `SUCCESS`.
- ARM job `101522384312` — `SUCCESS`.

The implementation checkpoint is `docs/checkpoints/OREGON_RUNTIME_JOURNAL_PROGRESS.md`.

## What Stage 4A still does not mean

- No VM execution is activated.
- No Stage 4B fee/receipt/runtime coordinator exists yet.
- No Stage 4C async delivery semantics are activated.
- No journal result is persisted or published into active consensus state.
- No existing transaction/block wire bytes are changed.
- EVM state is not routed through the Oregon SMT journal.
- No main integration has occurred.

The journal final result is an unpublished proposal only. Source state and active roots remain immutable through Stage 4A APIs.

## Immediate next action

This HANDOFF/checkpoint commit is a documentation successor to the verified implementation head. Verify the successor commit's own exact-head Rust CI and journal x86_64/ARM vectors, then update PR #19 with the closure evidence.

After that, **stop before main integration**. PR #19 remains draft until the separate owner integration decision required by the repository process. Do not silently merge or activate Stage 4A.

## Queued work after Stage 4A reaches main

Once Stage 4A is separately approved and successfully integrated to `main`, the default next verification/public-evidence program is recorded in `docs/checkpoints/OREGON_TRUST_VERIFICATION_ROADMAP.md`:

1. Formal 1:1 reserve-conservation proof design, with a Rust-compatible verifier such as Kani as the initial candidate and explicit proof boundaries/negative controls.
2. Reproducible public mutation evidence manifest/report for the existing consensus/runtime mutation gates. Mutation testing must not be mislabeled as formal proof.
3. RandomX public testnet/adversarial challenge preparation after node/testnet readiness; no unapproved monetary bounty or mainnet stress event.

This queued trust track does not erase the accepted Stage 4B/4C architecture.

## Preserved cautions

- `FundingCapabilityV1::new` validates data; it does not verify a live payer source. Future coordination must obtain authority from source validation.
- Existing `WeightMeter` exhaustion is sticky and charges the maximum. Child rollback cannot reset it or cumulative conversion counters.
- No generic VM-facing unrestricted system-domain write or non-revertible write API is permitted.
- Main integration remains a separate decision for every future slice.
- Never force-push or move accepted historical checkpoint refs.

Keep this record and the Stage 4A checkpoint in the repository so another conversation can continue without relying on chat memory.
