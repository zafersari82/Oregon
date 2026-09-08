# Oregon Reserve Conservation Proof V1 — Acceptance Checkpoint

Recorded: 2026-09-08 UTC.

## Status and scope

This checkpoint accepts the owner-approved **Reserve Conservation Proof V1** verification slice for the inactive Stage 3B execution-reserve foundation. It does not activate execution, reserve handling in the live block path, a VM, a protocol upgrade, or any new monetary rule.

The permitted claim is deliberately narrow:

> Kani verifies the listed reserve-conservation properties of Oregon's bounded reserve model; differential tests check correspondence with the inactive Stage 3B implementation.

The proof is not a mathematical refinement proof of all production Rust or all possible Oregon states.

## Authority

- Design: `docs/superpowers/specs/2026-09-06-reserve-conservation-proof-v1.md`.
- Implementation plan: `docs/superpowers/plans/2026-09-06-reserve-conservation-proof-v1.md`.
- Owner direction: `docs/architecture/OREGON_OWNER_DIRECTION.md`.
- Trust roadmap: `docs/checkpoints/OREGON_TRUST_VERIFICATION_ROADMAP.md`.
- Implementation branch: `work/reserve-proof-arithmetic-2026-09-08`.
- Pull request: #24, `Reserve proof: formal arithmetic obligations`.

## Exact verified proof source

The proof/model/control implementation accepted by this checkpoint was verified at:

- Source SHA: `511f61302ee486e618a33235808572ae4419f487`.
- Source tree: `e2f12792a08a5376807baa4e92b4ca63ef44a56b`.
- Base `main` at the verified PR state: `a530ae1d20e5a1648cf9cfea045221ec13326d25`.
- PR #24 was `mergeable=true`, `rebaseable=true`, `mergeable_state=clean` at that state.

The later documentation/workflow-label closure commit that contains this checkpoint must receive its own exact-head CI before integration. A green ancestor is not a substitute for that final closure-source verification.

## Pinned verifier and tool identity

The proof job verifies the committed tool lock before executing any accepted proof:

- Kani: `0.67.0`.
- Proof Rust toolchain: `nightly-2025-11-21-x86_64-unknown-linux-gnu`.
- Proof rustc: `rustc 1.93.0-nightly (53732d5e0 2025-11-20)`.
- CBMC: `6.8.0 (cbmc-6.8.0)`.
- Solver: bundled CaDiCaL `2.0.0`, selected explicitly by every proof harness.
- Verified Kani release archive size: `143387767` bytes.
- Verified Kani release archive SHA-256: `3b5f7afd3b51603ee720db7bc1bc4fe46b5a4f5d36daad9939c4b4c658b51ac0`.
- `bin/cbmc` SHA-256: `decfbd8336c557f663d539b3bb4b12aac3396009d616810d325bcd8b42b8a973`.
- `bin/kani-driver` SHA-256: `683f3ad1216e67686a39b2fcd6dc661f5090574f55fde9dd407966dca42cbfad`.
- `bin/kani-compiler` SHA-256: `f74488368997b6b4e0b417a2f5a3c8e598eccc907288c1a7875d9866056e9ffc`.

Relevant source digests at the verified proof source:

- `verification/reserve-conservation/proofs.rs`: `c098a4b8fbb6c6a2854f244cd775caa978578037e077d8887aa0a6b7b77f36e5`.
- `verification/reserve-conservation/src/model.rs`: `cf8d717639271acf454a39d2bece5bfa6542ea4e677838ab6772278f34545e3f`.
- `verification/reserve-conservation/toolchain-lock.json`: `7491cfb4808e9709e4d24750ef7ef32903393aeb627108437cdb85df659a844f`.
- `scripts/verify_reserve_proofs.py`: `ac1de1bd1c52418670e6e2a8fe5052f249e46b5bff54d5f8f1f4befce15fa18b`.
- `scripts/verify_reserve_mutation_controls.py`: `93778acad8d04df9af0ed1ea5872afc13031869faced6f0e213408bd8a03c93c`.

## Formal proof obligations

All ten named obligations passed on Linux x86_64 with the pinned verifier. State harnesses use explicit unwind bound `5`.

1. `rc01_arithmetic_matches_integer_equation` — accepted checked arithmetic equals the independent mathematical equation without wrap/saturation.
2. `rc02_result_matches_claimed_execution_total` — accepted reserve result equals the claimed execution total.
3. `rc03_zero_and_positive_reserve_cardinality` — accepted zero result has no reserve and positive result has exactly one modeled live reserve.
4. `rc04_multiple_live_reserves_reject_unchanged` — malformed two-reserve state rejects without publication.
5. `rc05_created_reserve_exact_entry` — successful creation has the exact modeled reserve program, value and metadata.
6. `rc06_failed_apply_and_undo_are_atomic` — failed apply/undo leave the modeled state unchanged.
7. `rc07_apply_then_undo_restores_full_state` — successful apply followed by its untampered undo restores the complete modeled pre-state using map-content equality rather than bounded-slot position.
8. `rc08_conservation_identity` — accepted transition satisfies the reserve arithmetic conservation identity as mathematical integers.
9. `rc09_invalid_arithmetic_and_endpoints_reject` — underflow, invalid previous snapshot and endpoint amount violations reject.
10. `rc10_collisions_and_tampered_undo_reject_unchanged` — occupied output keys and stale/tampered undo reject without state publication.

Required zero/positive/rejection/collision/undo reachability checks are part of the accepted harness inventory. The runner rejects empty or incomplete harness inventories, unsupported constructs, unexpected properties, wrong tool identity, timeouts, compilation failures and unwind failures.

## Negative controls

All ten required disposable-model mutation controls were killed by their selected semantic proof assertions with concrete Kani counterexamples; compilation failure or unrelated failure is not accepted as a kill:

1. `control_rc01_wrapping_add`.
2. `control_rc02_missing_execution_equality`.
3. `control_rc03_retain_zero_reserve`.
4. `control_rc04_remove_multiple_reserve_rejection`.
5. `control_rc05_create_ordinary_program_reserve`.
6. `control_rc06_remove_previous_before_collision`.
7. `control_rc07_omit_previous_restoration`.
8. `control_rc08_omit_fee_subtraction`.
9. `control_rc09_saturating_subtraction`.
10. `control_rc10_skip_created_entry_equality`.

After the final control, the runner verified that the baseline model digest and repository remained unchanged and reran the complete positive RC01–RC10 suite successfully.

## Exact-head proof evidence

For source `511f61302ee486e618a33235808572ae4419f487`:

- Oregon Reserve Conservation Proofs run `34220257918`, job `102041448968`: **SUCCESS**.
  - Fail-closed runner unit tests: SUCCESS.
  - Pinned installer/archive verification: SUCCESS.
  - RC01/RC02/RC08/RC09 arithmetic proofs: SUCCESS.
  - RC03/RC04/RC05/RC06/RC07/RC10 state proofs: SUCCESS.
  - RC01–RC10 mutation controls plus complete positive rerun: SUCCESS.
  - Evidence artifact ID `10053933077`, artifact digest `sha256:e84b33e5736ff9597baf6f0a2521bec3082ad0ca97da1188f4c1e6f35d303953`.
- Oregon Reserve Verifier Bootstrap run `34220257907`, job `102041448925`: **SUCCESS**. Positive / intended negative / positive smoke sequence and retained evidence passed with the pinned verifier.

## Production correspondence and inherited gates

The exact verification model source is compiled directly into `oregon-utxo` tests with:

`#[path = "../../../verification/reserve-conservation/src/model.rs"] mod reserve_model;`

At the same source, Oregon Rust CI run `34220257858`, job `102041448416` was **SUCCESS**. The full workspace/all-target run executed the production/model correspondence matrix, including constructor error precedence, intermediate overflow, endpoint bounds, zero/positive reserve cardinality, exact created metadata/program, multiple-reserve rejection, collision atomicity, tampered undo rejection and full apply/undo restoration.

The same Rust CI also passed architecture scans, focused contracts, workspace tests, rustdoc/docs, Format, warnings-denied Clippy and the inherited mutation authorities:

- execution addresses: `3/3` killed;
- execution envelope: `9/9` killed;
- contract state: `17/17` killed;
- execution resources: `13/13` killed;
- Stage 3B fee settlement/reserve: `14/14` killed;
- Stage 4A runtime journal: `12/12` killed.

The reserve-proof branch does not change the production Stage 3B reserve implementation. Accepted independent Stage 3B fee-settlement/reserve vector evidence remains run `34039255396`, with x86_64 job `101502890461` and ARM job `101502890592` both successful. This retained ARM evidence is executable-vector evidence for the unchanged production foundation; **Kani itself is not claimed to run on ARM**.

## Explicit non-proven scope

This checkpoint does **not** prove:

- production/model equivalence for all possible inputs;
- arbitrary or unbounded `UtxoState`, allocator behavior or `HashMap` implementation details;
- BLAKE3 collision resistance or arbitrary locking-program byte parsing;
- correctness or existence of caller-supplied account totals, authorizations or fee provenance;
- fee escrow correctness as a whole;
- deposit consumption, withdrawal output creation or producer-output creation;
- storage, RocksDB, WAL, crash/recovery or durable-publication behavior;
- block integration or atomic native/execution block transitions;
- cross-domain reorganization behavior;
- VM execution or runtime determinism;
- global native OREG supply conservation across every subsystem.

RC08 is conditional on the supplied flow totals. It proves the bounded reserve arithmetic identity, not that matching live economic flows exist elsewhere in the system.

## Integration boundary

The reserve-proof implementation has reached the technical acceptance boundary. The remaining repository workflow is:

1. commit this checkpoint, current `HANDOFF.md` and accurate CI labeling;
2. run all applicable exact-head CI on that documentation/workflow closure source;
3. obtain the repository-required separate explicit owner decision to integrate PR #24 into `main`;
4. after integration, verify the actual `main` merge source rather than treating PR evidence as main evidence.

After authorized integration, the trust roadmap continues with Workstream B: reproducible mutation-evidence publication. Stage 4B, Stage 4C, VM activation and the public RandomX testnet/challenge remain separately gated work.
