# Oregon — current continuation record

Updated: 2026-09-07 UTC. Stage 3B and Stage 4A are integrated into `main` as **inactive foundations**. Stage 4A remains unpublished/non-activated: it adds a bounded execution journal and verification infrastructure, not VM execution, persistence or protocol activation.

## Resume here

### September 8 continuation (supersedes older implementation status below)

- PR #22 remains open on `work/reserve-evidence-gate-2026-09-07`.
  Published source `ef5bca0e30d7486ff6d5e38d78fb31478f2fc255` has the exact
  tree of local `b64d96410439967801024c2428fad1d7f887be01`.
- At that source Rust CI #841, fee vectors #115 and bootstrap #69 succeeded.
  Proof Slices #28 (run 34160229290) passed both positive proof steps,
  accepted RC01-RC04 controls, then rejected RC05's UNREACHABLE cover status.
- This continuation adds counted UNREACHABLE cover handling for negative
  controls only. Positive runs still require all expected covers SATISFIED.
  Local verification: 29 Python parser tests pass; replay of the retained
  run-34160229290 artifact accepts RC01-RC05 controls and all ten positives.
  This replay is not a fresh Kani run and does not validate RC06-RC10 controls.
- Next: inspect new exact-source Proof Slices CI, resolve remaining failures,
  then complete the evidence manifest and correspondence matrix before any
  proof acceptance. Ten harnesses and ten control definitions now exist;
  older claims below that they are unimplemented are historical.
- Production semantics and main are unchanged. No integration decision is
  implied. The older dirty worktree and its correspondence changes are preserved.

- Current work branch: `work/reserve-evidence-gate-2026-09-07`, based on main
  `a530ae1d20e5a1648cf9cfea045221ec13326d25` (PR #21 integrated).
- PR #21 verified head: `51659a39dd0c41bbf607e75f2a4d5b3589e53f27`.
  Head and merge share tree `628f3b54e1a3c96c129368c66ad8f55991426afc`.
  See `docs/checkpoints/OREGON_RESERVE_TOOLING_MAIN_INTEGRATION.md` for exact CI.
- The standalone verification-only package, arithmetic/four-slot apply/undo model,
  and 12 crate-local production correspondence tests now exist. The model is
  included once under `cfg(test)`; the duplicate-mod wiring fix is integrated.
- RC01-RC10 Kani harnesses, full proof runner/manifest, production differential
  matrix completion and ten semantic model controls are still incomplete.
  No RC01-RC10 formal proof completion is claimed. Bootstrap is tooling evidence.
- This branch hardens bootstrap output acceptance: reject contradictory/repeated
  backend identities, property blocks outside RESULTS and extra completion lines.
  Local regression evidence: 14 new corrupted-output variants accepted by the old
  parser (semantic RED), then 11 test methods passed with the fix (GREEN).
  Retained logs: `verification/reserve-conservation/evidence/output-inventory/`.
- New branch CI must be checked at its exact published head; baseline CI does not
  validate this branch. Rust/Kani are absent locally; use the existing GitHub CI
  workflow for live pinned bootstrap and inherited Rust gates.
- Next: verify this parser slice in live pinned CI; then expand the constructor
  correspondence matrix and implement the first arithmetic proof harnesses with
  independent i128 assertions, reachability, named negative controls and exact
  evidence. Keep all ten obligations pending until their real artifacts exist.
- PR #20 remains separate Stage 4B work (observed head
  `912e9d01791c954e8432e3f80f6913e3b87c8db6`); do not import it into Workstream A.
- The approved reserve-proof design remains authorized. This new branch has no
  separate main-integration decision; do not merge it automatically.

## Historical September 7 pre-integration continuation

The following record describes the earlier runner/publication state, not current
pending work. The resume section above supersedes its status and next-action text.

- Published work branch: `work/reserve-proof-publication-2026-09-07`.
- Preserved local history branch: `work/reserve-proof-runner-2026-09-07`.
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


## September 7 connector publication and bootstrap CI

GitHub connector writes succeeded after Git CLI authentication failed. Source
snapshot `ed4e3b5af5c6e56d0404548866206766f902936d` has the exact original local
`976d6e5` tree `975454b9224584dd20e0b24fd614cb9bb6fc65fb`. This is a new publication
commit, not a claim that original local commit identities were pushed as Git refs.

The complete original history is preserved remotely on
`archive/reserve-continuation-2026-09-07` at
`01451b8136cd93e22497704537f1f2a56ec0443a`, in
`Oregon-continuation-2026-09-07.bundle`. Its Git blob identity
`c8c71e1b0bdb1b14714560a5e1571a5cb06db577` matches the local bundle bytes.

A dedicated `oregon-reserve-bootstrap.yml` CI workflow now prepares the pinned
verifier, checks the archive digest before setup, and executes the bootstrap-only
runner with positive/negative/positive checks and retained evidence. It does not
execute RC01–RC10. Inspect its actual run before claiming live validation. The
local Rust download was again stopped at network approval; CI is the intended
execution environment for this continuation.

Next: inspect and resolve bootstrap CI results, then continue the approved
standalone-package and test-first production-correspondence work. Main integration
and reserve-proof acceptance have not occurred.
