# Oregon Reserve Conservation Proof V1 — Implementation Plan

**Status:** owner-approved on 2026-09-06; start with proof-tool bootstrap.

**Base:** `dd7cdcb566273c39d5a38cf0c0036058b08a7d89`.

**Design:** `docs/superpowers/specs/2026-09-06-reserve-conservation-proof-v1.md`.

## September 7 continuation status

PR #21 integrated the bootstrap, standalone model package and 12 initial
correspondence tests at `a530ae1d20e5a1648cf9cfea045221ec13326d25` with successful
actual-main CI. See `docs/checkpoints/OREGON_RESERVE_TOOLING_MAIN_INTEGRATION.md`.
The original checklist below is not a claim that the package/runner are absent:
partial implementations now exist, but full obligation/matrix acceptance remains
open. The runner executes bootstrap only; RC01-RC10 remain unimplemented.
The current evidence-gate branch adds test-first output-inventory rejection and
updates HANDOFF. Its own exact-head CI and separate integration are required.

## 1. Resume and verify the baseline

- [x] Read `AGENTS.md`, `HANDOFF.md`, normative architecture, Stage 3B design and
  acceptance records, current reserve source and mutation runner.
- [x] Verify main source and query its current GitHub checks: seven returned jobs
  succeeded at the exact base above. Rust run `34048877702`; vector runs
  `34048877715`, `34048877699`, `34048877641` each include x86_64 and ARM.
- [x] Inspect open PRs: #20 is separate draft Stage 4B work at
  `fc2cc1564de69eee1482a445bb0c08b49b667ab0`; main's trust-roadmap order is unchanged.
- [x] Verify the Kani 0.67.0 tag against its repository source and inspect its
  installation, toolchain, backend and solver documentation.
- [x] Record owner approval of this model boundary and pinned verifier in
  `docs/architecture/OREGON_OWNER_DIRECTION.md`.

This design review ran without a local Rust toolchain. No local Rust, Kani,
mutation or newly authored proof execution is claimed by these checked items.

## 2. Establish reproducible proof tooling

On `work/reserve-conservation-proof-v1-2026-09-06`, based on the reviewed design:

- [ ] Add standalone verification package/model directory outside the production
  workspace and a committed lockfile if the package uses dependencies.
- [x] Record Kani 0.67.0, pinned source commit, nightly and CBMC versions from the
  design, plus downloaded asset and solver digests in `toolchain-lock.json`.
- [x] Verify a positive smoke assertion and an intentionally failing assertion;
  record real property identifiers/output shape for the result parser.
- [ ] Fail closed on missing tools, wrong versions, unexpected harness inventory,
  timeouts, compilation failures, unsupported operations and unwind failures.

Do not add Kani as a production dependency or change Rust 1.85.0. If this selected
release cannot run the bounded model, document the actual failure before proposing
a different version; do not silently weaken the proof or switch tools.

Local bootstrap evidence is in `verification/reserve-conservation/evidence/bootstrap/`.
At source `e816a9c`, the positive harness passed, its maximum-input cover was
satisfied, and the negative harness failed the intended assertion with input 255.
The positive harness passed again afterward. These are tooling smoke checks, not
RC01–RC10 results or exact-head CI acceptance. The installer required separate Rust
1.88.0; production stays at 1.85.0. The verified archive and binary digests are in
`verification/reserve-conservation/toolchain-lock.json`.

## 3. Write failing correspondence tests first

- [ ] Add crate-local reserve model comparisons under `cfg(test)` with no exported
  production hooks. Keep existing reserve tests and vectors unchanged.
- [ ] Pin constructor error precedence, intermediate-overflow rejection and
  endpoint-only supply bounds before writing the model implementation.
- [ ] Add the exact state shapes and malformed states listed in design sections
  5 and 7, including full-entry equality after failure and undo.
- [ ] Demonstrate the tests fail semantically against a deliberately incorrect
  model; a missing-module or compilation error is insufficient red evidence.

## 4. Implement the bounded model and prove the obligations

- [ ] Implement checked arithmetic and four-slot reserve state/undo in the
  verification package, preserving exact constructor/apply/undo behavior.
- [ ] Add explicitly named RC01–RC10 harnesses and reachability obligations.
- [ ] Use arbitrary full-width scalar amounts and independent `i128` assertions;
  keep collection/key/program abstraction limits explicit in the manifest.
- [ ] Run the same model through real production differential tests and existing
  independent reserve vectors. Diagnose disagreement before changing either side.
- [ ] Run Kani with explicit solver and per-harness unwind bounds, preserving all
  safety checks. Commit the actual successful bounds and invocation to the manifest.

The full runner surface extends the existing bootstrap-only repository command
`python3 scripts/verify_reserve_proofs.py`, that reads the committed manifest and
executes the exact discovered harnesses. Full RC01-RC10 mode does not exist yet.

## 5. Prove the negative controls are meaningful

- [ ] For each design control, mutate only a disposable model checkout, compile,
  and require the expected semantic property failure and counterexample.
- [ ] Reject compile errors, crashes, unsupported features, unwind failures,
  empty suites and unrelated assertion failures as control kills.
- [ ] Add parser regression fixtures from real positive, counterexample and
  infrastructure-failure outputs observed with the pinned verifier.
- [ ] Validate restoration and rerun the complete positive proof suite afterward.
- [ ] Keep the existing fee-settlement mutation runner as the production mutation
  authority; do not count model controls toward its 14/14 total.

## 6. Add CI and collect exact-source evidence

- [ ] Add a separate `oregon-reserve-proofs.yml` job on Linux x86_64 with pinned
  action commits, read-only permissions and a verified tool lock.
- [ ] Trigger on PRs to main and pushes to the implementation branch. Include
  production reserve/primitives, verification, scripts, lockfiles and workflow
  changes in any path filters, or omit path filtering entirely.
- [ ] Run positive proofs, reachability checks and negative controls. Upload raw
  logs, source/tool identities, counterexamples and machine-readable results.
- [ ] Run production correspondence tests on existing supported Rust CI and retain
  x86_64/ARM vector evidence; do not claim ARM Kani proof execution.
- [ ] Run all inherited gates required by the constitution:

```bash
cargo +1.85.0 test --locked --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings
python3 scripts/generate_fee_settlement_vectors.py --check
python3 scripts/verify_fee_settlement_mutations.py
```

Retain the architecture scan, other inherited mutation gates, rustdoc and docs
steps already present in Rust CI. Do not substitute a green ancestor for the
final candidate's verification.

## 7. Checkpoint and integration boundary

- [ ] Record exact head/tree, actual verifier/tool digests, all successful proof
  IDs and bounds, control results, differential evidence and CI URLs in a new
  reserve-proof checkpoint.
- [ ] State that the model is formally checked and correspondence is tested;
  preserve every non-proven scope item in the design.
- [ ] Update `HANDOFF.md` with the accepted proof slice's actual remaining action.
- [ ] Obtain the separate explicit main-integration decision before merging.

After acceptance, continue with the roadmap's mutation-evidence publication.
Stage 4B, Stage 4C and a public testnet remain separately gated work.
