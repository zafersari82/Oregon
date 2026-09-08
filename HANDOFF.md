# Oregon — current continuation record

Updated: 2026-09-06 UTC. Stage 3B and Stage 4A are integrated into `main` as **inactive foundations**. Stage 4A remains unpublished/non-activated: it adds a bounded execution journal and verification infrastructure, not VM execution, persistence or protocol activation.

## Resume here

- Current local work branch: `work/reserve-model-2026-09-07`;
  publication target remains `work/reserve-conservation-proof-v1-2026-09-06`.
- Owner approved Reserve Conservation Proof V1 after reviewing design commit
  `7743aaa`; implementation is authorized. Do not request the same approval again.
- Read `docs/architecture/OREGON_OWNER_DIRECTION.md` for the persistent engineering
  objective, future subsystem assurance needs and limits of this reserve proof.
- Current completed slice: pinned verifier bootstrap at source `e816a9c`. Kani
  0.67.0/CBMC 6.8.0/CaDiCaL 2.0.0 passed the positive smoke check and found the
  intended negative counterexample (255); the positive rerun also passed.
- Evidence: `verification/reserve-conservation/evidence/bootstrap/summary.json` and
  raw logs. These are local tool checks, not reserve proofs or CI acceptance.
- Fail-closed runner/standalone package slice is committed locally at `448e516`.
  Its 21 Python behavior tests pass, and the default command rejects the incomplete
  RC01–RC10 suite with exit 2 before tool lookup. Real bootstrap execution was not
  repeated in that environment because Rust, rustup and Kani were unavailable.
- Current next action: write the approved crate-local production correspondence
  tests before the bounded model, beginning with a semantic RED against a deliberately
  incorrect model. A Rust-capable environment is required to observe that RED.
  RC01–RC10 remain unimplemented; do not count bootstrap harnesses as reserve proofs.
- Publication verified: local `448e516` and `6453525` were reproduced on GitHub
  as `ce118b6` and `2471aef`, with identical respective tree identities. Original
  local history remains on `archive/reserve-runner-local-6453525`.
- 2026-09-07 continuation: Rust 1.85.0, rustfmt and Clippy are now installed.
  Three constructor characterization tests pin error precedence, endpoint-only
  supply checks and intermediate-overflow rejection. No production behavior changed.
  Local checks: 13 reserve tests, all 35 UTXO unit tests plus one independent
  vector test, 21 runner tests, workspace format and UTXO Clippy passed.
  These are characterization tests, not model correspondence or Kani proofs.
- Full workspace test attempt was blocked in the RocksDB build by missing
  libclang. Kani is not installed in this environment. Exact-head CI and the
  bounded-model semantic RED remain pending; no acceptance checkpoint is claimed.
- The Stage 3B mutation runner stopped before injecting any mutation: its
  execution-fee vector baseline could not compile (`E0463`, missing `serde` crate).
  This is unresolved build evidence, not a killed mutation or a production
  regression diagnosis. Reproduce with a fresh build directory before proceeding.

### 2026-09-08 environment recovery and CI continuation

- Constructor tests are published at `991a5ff35ba14a1f44993ae765c5c7847284b5c9`.
- The identical execution fee vector tests passed 2/2 in a fresh target directory,
  without source changes. The earlier `serde` error was not reproduced there;
  stale build artifacts are suspected, not a demonstrated source defect.
- libclang 18.1.1 was installed, but the next session lost the installed Rust
  toolchains and active process handles. The restarted mutation run's final result
  was not recovered. Do not claim 14/14 or successful full-workspace verification.
- Oregon Rust CI now includes the reserve publication branch and the 21 runner
  behavior tests. Existing Rust 1.85.0, native prerequisites, workspace checks and
  inherited mutations remain in the same workflow. CI results are pending.
- Next action: inspect exact-head CI and resolve any observed failures, then add
  the approved bounded model correspondence tests and semantic RED. Kani
  RC01–RC10 and the separate proof workflow remain unimplemented.

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
