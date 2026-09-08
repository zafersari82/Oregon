# Oregon Reserve Conservation Proof V1 — Implementation Plan

**Status:** owner-approved on 2026-09-06; implementation and proof acceptance complete on the isolated branch; final documentation/workflow closure must pass exact-head CI before the separate `main` integration decision.

**Base:** `dd7cdcb566273c39d5a38cf0c0036058b08a7d89`.

**Design:** `docs/superpowers/specs/2026-09-06-reserve-conservation-proof-v1.md`.

## 1. Resume and verify the baseline

- [x] Read `AGENTS.md`, `HANDOFF.md`, normative architecture, Stage 3B design and acceptance records, current reserve source and mutation runner.
- [x] Verify main source and its exact-source checks before implementation.
- [x] Inspect separate open Stage 4B work and preserve the trust-roadmap priority.
- [x] Verify the Kani 0.67.0 tag/source and installation/toolchain/backend/solver identities.
- [x] Record owner approval of this model boundary and pinned verifier in `docs/architecture/OREGON_OWNER_DIRECTION.md`.

The original design review did not claim proof execution. Live pinned proof evidence was established later in dedicated CI and is recorded in `docs/checkpoints/OREGON_RESERVE_CONSERVATION_PROOF_V1.md`.

## 2. Establish reproducible proof tooling

- [x] Keep the standalone verification package/model under `verification/reserve-conservation/`, outside the production Cargo workspace, with committed tool identity data.
- [x] Record Kani 0.67.0, pinned source/toolchain/backend/solver and verified release/binary digests in `toolchain-lock.json`.
- [x] Verify a positive smoke assertion and intentionally failing assertion and pin their real Kani output/property shape.
- [x] Fail closed on missing/wrong tools, unexpected harness inventory, timeouts, compilation failures, unsupported operations and unwind failures.

Kani remains verification-only. Oregon production Rust remains 1.85.0; the installer/proof toolchains are separate.

## 3. Write failing correspondence tests first

- [x] Add crate-local model comparisons under `cfg(test)` with no exported production verification API.
- [x] Pin constructor error precedence, intermediate-overflow rejection and endpoint-only supply bounds before model acceptance.
- [x] Cover the required state/malformed shapes, including full-entry equality after failure and undo.
- [x] Preserve semantic RED evidence using deliberately weakened behavior rather than treating compilation errors as test evidence.

The same model source is compiled by `oregon-utxo` tests through the explicit `#[path = "../../../verification/reserve-conservation/src/model.rs"]` binding.

## 4. Implement the bounded model and prove the obligations

- [x] Implement checked arithmetic and four-slot reserve state/undo with the accepted production semantics.
- [x] Add explicitly named RC01–RC10 Kani harnesses and reachability obligations.
- [x] Use full-width symbolic scalar amounts and independent mathematical assertions; keep collection/key/program abstraction limits explicit.
- [x] Run the same model through production differential tests and existing independent reserve vectors; diagnose and correct semantic disagreement before acceptance.
- [x] Run Kani with explicit CaDiCaL selection and explicit state unwind bound while preserving safety/unwind checks.

The repository runner surfaces are `scripts/verify_reserve_proofs.py` and `scripts/verify_reserve_mutation_controls.py`.

## 5. Prove the negative controls are meaningful

- [x] Execute all ten design controls in disposable model copies and require the selected semantic proof assertion to fail with a concrete counterexample.
- [x] Reject compilation errors, crashes, unsupported features, unwind failures, empty suites and unrelated assertion failures as mutation kills.
- [x] Maintain parser regression fixtures from observed positive, counterexample and infrastructure-failure output.
- [x] Verify the baseline model/repository is restored after controls and rerun the complete positive RC01–RC10 suite afterward.
- [x] Keep the production Stage 3B fee-settlement mutation runner as the separate 14/14 production mutation authority.

## 6. Add CI and collect exact-source evidence

- [x] Add separate `oregon-reserve-proofs.yml` Linux x86_64 CI with pinned action commits, read-only permissions and verified tool lock.
- [x] Trigger on PRs to `main` and pushes to the implementation branch without weakening relevant-source coverage through path filtering.
- [x] Run positive proofs, reachability checks and all ten negative controls and retain raw/machine-readable evidence.
- [x] Run production correspondence tests through supported Rust CI and preserve independent production vector evidence; do not claim ARM Kani execution.
- [x] Run inherited architecture, full workspace/all-target, mutation, rustdoc/docs, Format and warnings-denied Clippy gates.

Accepted proof-source evidence at `511f61302ee486e618a33235808572ae4419f487`:

- Reserve Conservation Proofs run `34220257918`, job `102041448968`: SUCCESS.
- Reserve Verifier Bootstrap run `34220257907`, job `102041448925`: SUCCESS.
- Oregon Rust CI run `34220257858`, job `102041448416`: SUCCESS.
- Proof artifact ID `10053933077`, digest `sha256:e84b33e5736ff9597baf6f0a2521bec3082ad0ca97da1188f4c1e6f35d303953`.

## 7. Checkpoint and integration boundary

- [x] Record exact verified proof source/tree, tool identities/digests, all RC01–RC10 proof IDs and bounds, ten control results, correspondence evidence and CI identifiers in `docs/checkpoints/OREGON_RESERVE_CONSERVATION_PROOF_V1.md`.
- [x] State the exact bounded-model proof claim and preserve the explicit non-proven scope.
- [x] Update `HANDOFF.md` to the current proof-acceptance and closure state.
- [ ] Obtain the separate explicit `main` integration decision and merge only after the final documentation/workflow closure source itself is exact-head green.

After authorized integration and verification of the actual `main` merge source, continue with the trust roadmap's Workstream B mutation-evidence publication. Stage 4B, Stage 4C and the public RandomX testnet/challenge remain separately gated work.
