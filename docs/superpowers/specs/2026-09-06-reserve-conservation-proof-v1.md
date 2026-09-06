# Oregon Reserve Conservation Proof V1

**Status:** proposed verification design; owner review required before implementation.

**Date:** 2026-09-06 UTC.

**Reviewed main:** `dd7cdcb566273c39d5a38cf0c0036058b08a7d89`.

**Authority:** `AGENTS.md`, the Engineering Constitution, the Platform Architecture
Contract, Execution Architecture V1, Stage 3B fee-settlement/reserve V1, and
`docs/checkpoints/OREGON_TRUST_VERIFICATION_ROADMAP.md`.

## 1. Decision and scope

Implement Workstream A first, using a small verification-only Rust model of the
existing inactive reserve transition. Prove the model with pinned Kani and check
its correspondence with the production implementation through differential tests.
The proof is of the explicitly bounded model; differential testing is evidence
of correspondence, not a mathematical refinement proof of production Rust.

This slice changes no production transition, amount rule, dependency, API, wire
encoding or activation. It does not continue the open Stage 4B PR #20, whose
runtime work remains separate. It does not claim conservation is already proven.

The owner-requested roadmap authorizes preparation of this design. It does not
record approval of this particular abstraction or verifier pin. Implementation
begins after that specific decision is recorded, following `AGENTS.md`.

## 2. Authoritative owners

`oregon-utxo/src/reserve.rs` remains the only production owner of reserve
construction, application and undo. `oregon-primitives` owns `Amount`, the reserve
locking-program bytes and identity derivation. No VM, storage or execution-layer
dependency is added to either owner.

Place the verification-only model, harnesses and tool manifest under
`verification/reserve-conservation/`, outside the production Cargo workspace.
Compile the same model source into crate-local reserve differential tests under
`cfg(test)` so those tests can call the existing private apply/undo functions.
There is no public test API, production fallback or replacement reserve algorithm.

Production sources reviewed at the base above have these Git blob identities:

| Source | Blob |
| --- | --- |
| `crates/oregon-utxo/src/reserve.rs` | `3d9ae3368ed6d5881f1ada78fd1cc2fa41e6a45d` |
| `crates/oregon-primitives/src/amount.rs` | `97714527e919fd74166dcc4219c1b72608550330` |
| `crates/oregon-primitives/src/execution_reserve.rs` | `ddaa6a3097330feafd071e2ba5ed085c59339489` |

These identify the review baseline, not an automatic proof of correspondence.
Later relevant production changes require renewed model review and differential
evidence; a fingerprint update alone cannot close that review.

## 3. Fixed verifier selection

Select Kani **0.67.0**, source tag `kani-0.67.0`, commit
`4feaaad1d6a2378a6ff6caa3b4fc5d6999c7bb5d` in
<https://github.com/model-checking/kani>.
Its checked-in `rust-toolchain.toml` pins `nightly-2025-11-21` and
`kani-dependencies` names CBMC **6.8.0**. Select the bundled **CaDiCaL** solver
explicitly in each harness; do not follow a floating solver default.

Use a separate Linux x86_64 proof job; leave Oregon's Rust 1.85.0 gates intact.
Bootstrap with `cargo install --locked kani-verifier --version 0.67.0` and
`cargo kani setup`. Before running proofs, commit a tool-lock manifest containing
the selected release asset's SHA-256, platform, compiler/backend versions and
solver binary digest. Verify downloads against that lock, and reject mismatches.
The bootstrap task must establish these digests before any proof acceptance;
this design does not invent unobserved release digests or claim a successful install.

Retain overflow, memory-safety and unwinding checks. A timeout, unsupported
operation, unwind failure or compiler error is an infrastructure/proof failure,
never a successful proof or a killed negative control. Increasing a loop bound
requires a manifest change and rerun; disabling unwinding assertions is forbidden.

## 4. Arithmetic model and exact acceptance semantics

Inputs are a previous snapshot presence flag and amount, deposits `D`, withdrawals
`W`, execution-funded fees `F`, and claimed resulting execution total `E`.
Every amount is symbolic over its entire `u64` range. Absent previous state means
`P = 0`; present previous state with amount zero is invalid.

Preserve the constructor's exact operation and error order:

1. Reject a present zero previous snapshot.
2. Compute checked `u64` addition `A = P + D`.
3. Compute checked `u64` subtraction `B = A - W`.
4. Compute checked `u64` subtraction `R = B - F`.
5. Require `R == E`.
6. Require both the present previous amount and `R` to fit `Amount`.

Use an independent `i128` mathematical expression for proof assertions, so the
assertion does not repeat the same checked-arithmetic implementation. All sums
and differences of these four `u64` operands fit `i128`.

Do not assume valid inputs before testing rejection. In particular, an overflowing
intermediate `P + D` must be rejected even if later subtractions would produce a
small mathematical result. The existing constructor checks endpoint amounts;
this proof must not silently impose a new supply cap on each flow or intermediate.
Fee provenance and correctness of the caller-supplied `E` are outside this model.

## 5. Bounded state model

Use four optional map slots with distinct symbolic `u8` key tokens. Each entry
contains an amount, creation height, coinbase flag and program classification.
The program classification is either exact reserve program or ordinary program;
production differential fixtures bind the reserve case to all canonical bytes
and include noncanonical ordinary programs. The model does not prove arbitrary
locking-program parsing or byte encoding.

Four slots permit zero/one/two live reserves, an ordinary collision at the proposed
output and an unrelated ordinary entry. Application removes at most one entry
and adds at most one. For zero-to-positive creation, start with at most three
occupied slots so capacity failure is not invented as a production validity rule.
Enumerate slot positions and key-equality patterns; no claim extends to arbitrary
`HashMap` size, allocator behavior or iteration implementation.

The derived output key is an input token whenever `R > 0`, and is absent exactly
when `R == 0`. It may equal a currently occupied key, including the old reserve
key; collision must reject before removal, exactly as production does. No
collision-freedom assumption is permitted. Hash derivation and cryptographic
properties remain covered by existing canonical vectors, outside this proof.

Apply validates singleton and previous entry matching, prepares the new entry
with exact reserve classification, height and `is_coinbase = false`, and then
publishes a complete copied model state. Undo stores full previous/created entries,
validates them against current state, and publishes only after all checks succeed.
Production comparison includes all ordinary entries and metadata, not just totals.

## 6. Proof obligations and negative controls

| ID | Required model property | Required weakened-rule control |
| --- | --- | --- |
| RC01 | Accepted arithmetic equals the mathematical equation without wrap/saturation | Replace checked addition with wrapping addition |
| RC02 | Accepted `R` equals claimed `E` | Remove equality rejection |
| RC03 | Accepted zero result has no reserve; positive result has exactly one | Retain/create a zero reserve |
| RC04 | Two live reserves reject unchanged | Remove multiple-reserve rejection |
| RC05 | Successful creation has exact modeled program, value and metadata | Create an ordinary-program reserve |
| RC06 | Failed apply and failed undo leave every entry unchanged | Remove the old reserve before collision validation |
| RC07 | Apply followed by its untampered undo restores the whole pre-state | Omit previous-entry restoration |
| RC08 | `R + W + F = P + D` as mathematical integers for every accepted transition | Omit execution-fee subtraction |
| RC09 | Underflow, invalid previous snapshot and endpoint amount violations reject | Replace checked subtraction with saturation |
| RC10 | Occupied output keys and stale/tampered undo reject unchanged | Skip created-entry equality validation in undo |

Assertions must be checked without assuming their own postconditions. Add explicit
reachability checks for zero, positive, rejection, collision and undo branches.
Nonempty harness discovery and a reachable accepted transition are mandatory,
so an always-rejecting model or contradictory assumption cannot appear proven.

Controls modify the verification model in disposable worktrees. They must compile
and fail the named semantic assertion with a retained counterexample. No broad
"nonzero exit means killed" policy is allowed. Restore/verify the baseline after
each control and rerun the positive suite after the final control.

## 7. Production correspondence and non-proven scope

Differential tests compare constructor success/error category, amount, output
presence, apply result, full resulting state, undo failure atomicity and round
trips. Cover `0`, `1`, supply limit and limit + 1, `u64::MAX`, intermediate overflow,
withdrawal/fee underflow, total mismatch, stale snapshot, double reserves, occupied
new outpoint, canonical/wrong program and tampered undo. Use existing independent
fee-settlement vectors in addition to deterministic generated cases.

Existing Stage 3B 14/14 mutation gates remain mandatory. Their results are test
evidence and must be reported separately from model proof controls.

Not proven here: production/model equivalence for all inputs; BLAKE3 collision
resistance; unbounded `UtxoState`; actual account totals and authorization; fee
escrow correctness; deposit consumption, withdrawal or producer-output creation;
storage/WAL/crash behavior; block integration; reorganization across domains;
VM execution; global native supply conservation. RC08 proves a reserve arithmetic
identity conditional on supplied flows, not the existence of matching live outputs.

## 8. Evidence and completion

Emit machine-readable results with schema version, exact source SHA/tree, model
and production source digests, tool lock, harness inventory, assumptions, bounds,
command, semantic property results, reachability results, control counterexamples,
differential results and CI run/job URLs. Missing expected harnesses or evidence
fail the gate. Preserve raw logs alongside the summary.

After implementation, require positive proofs, all controls, production
correspondence tests, inherited mutation gates and standard workspace checks on
the exact candidate head. Record a new checkpoint with explicit proven/non-proven
scope. Integrating into `main` remains a separate owner decision.

Permitted eventual claim: "Kani verifies the listed reserve-conservation properties
of Oregon's bounded reserve model; differential tests check correspondence with
the inactive Stage 3B implementation." No proof claim is permitted before those
artifacts actually pass.
