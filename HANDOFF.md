# Oregon — current continuation record

Updated: 2026-09-07 UTC. Stage 3B and Stage 4A are integrated into `main` as **inactive foundations**. Stage 4A remains unpublished/non-activated: it adds a bounded execution journal and verification infrastructure, not VM execution, persistence or protocol activation.

## Resume here

- Current local work branch: `work/reserve-proof-runner-2026-09-07`.
- Owner-approved Reserve Conservation Proof V1 remains authorized; do not request
  the same approval again. All original continuation commits through `6a7770b`
  are preserved as ancestors.
- Latest code slice: `0691c44`, fail-closed bootstrap result parser and process runner
  in `scripts/verify_reserve_proofs.py`. Six Python test methods pass, including
  19 corrupted-output cases and timeout/missing-tool/compile-failure rejection.
  Semantic red evidence preceded implementation. Evidence is recorded in
  `verification/reserve-conservation/evidence/runner/`.
- Historical live bootstrap remains source `e816a9c`, Kani 0.67.0 / CBMC 6.8.0 /
  CaDiCaL 2.0.0. No live Kani execution occurred in the September 7 continuation.
- Current environment blocker: Rust/Kani executables are absent; the Rust download
  attempt was stopped at network approval. Live pinned preflight/invocation and
  inherited Rust/Clippy/mutation gates are unvalidated for the new code slice.
- Next incomplete action: validate the bootstrap runner with the pinned archive,
  installed Kani and nightly rustc, then add the standalone package and write the
  approved production correspondence tests before implementing the reserve model.
  The full RC01–RC10 runner, model, controls and proof CI remain unimplemented.
- On September 7 the live GitHub main was checked at `dd7cdcb` with seven successful
  checks at that exact source. PR #20 remains a separate Stage 4B draft. Those checks
  do not validate the new local runner.
- Publication status: see the latest continuation publication record below.

- Repository: `zafersari82/Oregon`.
- Stage 3B main integration: `fe762f7a5670d94a486423327e3a525cec24afb5`.
- Stage 4A main integration: `dee4ca6ea3b6dc75e4920ab56a44e2b5da8aa0a3`.
- Stage 4A integration tree: `02da551eba74ee53ab003364c6a054176c02755f`.
- Stage 4A closure source/checkpoint head: `bb24a2cb1bed17736c931888b334388a67f29842`.
- Merged PR: #19 — `Stage 4A: bounded execution journal foundation`.
- Stage 4 design: `docs/superpowers/specs/2026-09-06-runtime-journal-async-v1-design.md`.
- Stage 4A plan: `docs/superpowers/plans/2026-09-06-runtime-journal-v1.md`.
- Stage 4A implementation checkpoint: `docs/checkpoints/OREGON_RUNTIME_JOURNAL_PROGRESS.md`.
- Stage 4A main integration record: `docs/checkpoints/OREGON_STAGE4A_MAIN_INTEGRATION.md`.
- Post-Stage-4A trust roadmap: `docs/checkpoints/OREGON_TRUST_VERIFICATION_ROADMAP.md`.

Always inspect the actual current `main`, open work branches and exact-head CI before editing; this record is a continuation aid, not a substitute for repository state.

## Integrated foundation

M0–M6, execution addresses, universal-envelope/auth structure, logical contract state, Stage 3A resource metering/base fees, Stage 3B escrow/settlement/reserve foundations and Stage 4A bounded runtime journal are integrated.

Stage 4A provides the approved journal surface in `oregon-execution`: validated Oregon SMT snapshot domains, checked overlay reads/writes, bounded nested frames, child commit/revert with ancestor rollback, exact structural/resource ceilings, atomic unpublished multi-domain finalization, independent trace vectors and dedicated mutation gates.

Stage 4A integration is **not activation**. The final journal result remains an unpublished proposal; active roots/source state are not persisted or changed by the journal APIs.

## Stage 4A integration evidence

Verified closure/checkpoint source:

- SHA `bb24a2cb1bed17736c931888b334388a67f29842`, tree `02da551eba74ee53ab003364c6a054176c02755f`.
- Rust CI #772 — run `34047120052`, job `101524055508` — SUCCESS.
- Runtime Journal Vectors #25 — run `34047120019`; x86_64 job `101524055523`, ARM job `101524055354` — SUCCESS.

Actual main merge:

- Merge SHA `dee4ca6ea3b6dc75e4920ab56a44e2b5da8aa0a3`.
- Merge tree `02da551eba74ee53ab003364c6a054176c02755f`, identical to the verified checkpoint tree.
- Rust CI #773 — run `34048254431`, job `101527099485` — SUCCESS.
- Runtime Journal Vectors #26 — run `34048254365`; x86_64 job `101527099240`, ARM job `101527099317` — SUCCESS.
- Execution Resource Vectors #103 — run `34048254372`; x86_64 job `101527099230`, ARM job `101527099377` — SUCCESS.
- Fee Settlement Vectors #52 — run `34048254591`; x86_64 job `101527099826`, ARM job `101527099958` — SUCCESS.
- Main Rust CI passed architecture/focused contracts, full workspace/all-target tests, mutation gates (address 3/3, envelope 9/9, contract-state 17/17, resource 13/13, fee-settlement 14/14, journal 12/12), rustdoc/docs, Format and warnings-denied Clippy.

No RandomX main-push rerun is claimed for this integration; the main push triggered Rust, runtime-journal, execution-resource and fee-settlement workflows. Earlier exact closure-source RandomX architecture/full-light parity evidence remains historical evidence, not a substitute for a nonexistent main-only rerun.

## What Stage 4A still does not mean

- No deterministic VM/runtime call ABI is active.
- No Stage 4B transaction/fee/receipt coordinator exists yet.
- No Stage 4C async delivery/expiry/consumption behavior is active.
- No journal proposal is persisted into active consensus state.
- No existing transaction/block wire bytes are changed by Stage 4A.
- EVM state is not routed through the Oregon SMT journal.
- No universal-envelope/VM execution activation is implied by this merge.

## Next owner-requested work

Stage 4A has reached `main`, so open `docs/checkpoints/OREGON_TRUST_VERIFICATION_ROADMAP.md` before selecting the next implementation task.

Default execution order, unless the owner explicitly reprioritizes:

1. **Formal Stage 3B 1:1 reserve-conservation proof design and implementation**, with a Rust-compatible verifier (Kani is the initial candidate), pinned reproducible CI, explicit proof boundaries and negative controls.
2. **Reproducible mutation evidence publication** generated/cross-checked from machine-readable repo evidence; mutation testing is not formal proof.
3. **RandomX public testnet/adversarial challenge preparation**, only after node/testnet readiness; no unapproved monetary bounty and no mainnet stress event.

The accepted Stage 4B/4C architecture remains valid but is not the default immediate work while this owner-requested trust track is queued.

## Preserved cautions

### Approved reserve-proof design

The branch `design/reserve-conservation-proof-v1-2026-09-06` prepares Workstream A
from main `dd7cdcb566273c39d5a38cf0c0036058b08a7d89`:

- Design: `docs/superpowers/specs/2026-09-06-reserve-conservation-proof-v1.md`.
- Plan: `docs/superpowers/plans/2026-09-06-reserve-conservation-proof-v1.md`.
- State: owner-approved bounded-model proof with Kani 0.67.0 and production
  differential tests; verifier bootstrap works locally, reserve proofs remain pending.
- Next action: execute the approved test-first plan on
  `work/reserve-conservation-proof-v1-2026-09-06`, continuing after the recorded
  bootstrap checks.

Open draft PR #20 remains separate Stage 4B work at
`fc2cc1564de69eee1482a445bb0c08b49b667ab0`; it does not supersede main's default
trust-roadmap priority. Neither that branch nor historical checkpoints were moved.

### Existing implementation cautions

- `FundingCapabilityV1::new` validates data; it does not verify a live payer source. Future coordination must obtain authority from source validation.
- Existing `WeightMeter` exhaustion is sticky and charges the maximum. Child rollback cannot reset it or cumulative conversion counters.
- No generic VM-facing unrestricted system-domain write or non-revertible write API is permitted.
- Future Stage 4B, Stage 4C and activation changes each require their own design/test/CI/checkpoint/integration decisions.
- Never force-push or move accepted historical checkpoint refs.

Keep this record and the integration checkpoint in the repository so another conversation can resume from exact evidence rather than chat memory.

## September 7 publication record

Push of `0691c44` and its preserved ancestors to
`work/reserve-proof-runner-2026-09-07` failed: Git HTTPS could not read a username
with terminal prompts disabled. GitHub read access works, but this session has no
usable Git write credential. No remote branch creation or current-head CI is claimed.
Continue from the local branch or its exported continuation bundle; main is unchanged.
