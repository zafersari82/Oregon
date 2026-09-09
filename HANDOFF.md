# Oregon — current continuation record

Updated: 2026-09-09 UTC.

## Resume here

- Repository: `zafersari82/Oregon`.
- Active local branch: `work/runtime-coordinator-v1-2026-09-06`, draft PR #20.
- Verified remote Stage 4B head remains `912e9d01791c954e8432e3f80f6913e3b87c8db6`.
- Main PR #25 is merged at `cc791386899a777344404b98278cf41f0752f173`.
- Local reconciliation commit: `83dc4653233851b14fa4658031fb5995089efc96`.
- Local unverified Task 8 implementation: `02d44e4`.
- Plan: `docs/superpowers/plans/2026-09-06-runtime-coordinator-v1.md`.
- Spec: `docs/superpowers/specs/2026-09-06-runtime-coordinator-v1-design.md`.

Tasks 1–7 exist in prior branch history. Task 8 adds two-phase receipt proposal
composition and tests, but is NOT accepted: Rust compilation/tests, formatting,
Clippy and independent review closure remain outstanding. The independent Python
runtime vector check and git whitespace check succeeded locally.

The local environment has no working Cargo toolchain. A command failing with
`cargo: command not found` is an environment failure, not semantic RED evidence.
Do not describe Task 8 as test-verified or reproduce that earlier claim.

Push to the existing Stage 4B branch was rejected by automatic approval review:
it could not establish authorization for publishing code to the destination.
No remote update occurred; do not bypass this rejection. Obtain explicit push
authorization before retrying. Preserve local commits and verify exact-head CI
once uploaded. Main integration remains a separate decision.

Independent Task 8 static review: CHANGES REQUIRED. Two P1 findings:

1. Add one owning coordinator path composing validated funding/escrow, runtime
   dispatch, settlement, real finalized Phase A, surviving effects and Phase B.
   Current proposal tests manufacture inputs and do not prove that integration.
2. Bind journal chain/height/txid and receipt execution domain to the validated
   runtime/funding context. Add negative tests changing each independently.

Next action: write/run regression tests for these findings before fixing them,
obtain genuine Rust verification, then continue Tasks 9–11. Do not skip straight to Task 10:
Task 8/9 were absent at the old remote head even though the earlier progress map
highlighted only the missing mutation workflow and checkpoint.

## Overall architecture and subsequent work

Do not report an overall completion percentage without a reviewed work estimate.
The source-backed platform status separates accepted main foundations, draft
implementations, production integration and activation.

Stage 4B code is present on PR #20 at
`912e9d01791c954e8432e3f80f6913e3b87c8db6`; its old description understates progress.
Its 16-mutation runner, dedicated x86_64/ARM workflow and acceptance checkpoint
remain absent. Reconcile that branch with current main before completing its
remaining gates. Do not mix Stage 4B into this publication PR.

The trust roadmap's Workstream C remains readiness-gated: M6 is not a runnable
public node/miner launch package. Preserve the agreed work order or an explicit
owner reprioritization. A/B completion does not authorize public testnet launch,
bounties, VM activation or changes to frozen architecture.

## Claim boundary

Evidence covers 68 selected injected faults across six authorities. It is not
formal proof, exhaustive fault coverage, or proof of absence of bugs. Reserve
model controls are separate. No production/runtime activation is included.

## Reserve Conservation Proof V1 — accepted foundation

The reserve proof/model/control implementation was accepted at exact source:

- SHA `511f61302ee486e618a33235808572ae4419f487`.
- Tree `e2f12792a08a5376807baa4e92b4ca63ef44a56b`.

At that source:

- Oregon Reserve Conservation Proofs run `34220257918`, job `102041448968`: **SUCCESS**.
- Oregon Reserve Verifier Bootstrap run `34220257907`, job `102041448925`: **SUCCESS**.
- Oregon Rust CI run `34220257858`, job `102041448416`: **SUCCESS**.
- Proof artifact ID `10053933077`, digest `sha256:e84b33e5736ff9597baf6f0a2521bec3082ad0ca97da1188f4c1e6f35d303953`.

The proof CI verifies pinned Kani 0.67.0 / CBMC 6.8.0 / CaDiCaL 2.0.0 identities before execution. All RC01–RC10 positive proof harnesses pass. All ten required disposable-model negative controls are semantically killed with concrete counterexamples, baseline source restoration is checked, and the complete positive RC01–RC10 suite is rerun afterward.

The state proof harnesses use explicit unwind bound `5`. Kani proof execution is Linux x86_64 only; do not claim ARM Kani verification.

`oregon-utxo` compiles the exact verification model source through:

`#[path = "../../../verification/reserve-conservation/src/model.rs"] mod reserve_model;`

Exact-head Rust CI runs the production/model correspondence tests. The RC07 model equality issue found during formal verification was resolved in the verification model by comparing bounded state as map content rather than slot position, matching production `HashMap` state semantics. Production reserve behavior was not changed by the proof slice.

Permitted proof claim:

> Kani verifies the listed reserve-conservation properties of Oregon's bounded reserve model; differential tests check correspondence with the inactive Stage 3B implementation.

Do not broaden this into proof of all production Rust, arbitrary/unbounded UTXO state, storage/WAL/crash behavior, block integration, cross-domain reorganization, VM execution, authorization/account-total provenance, or global native OREG supply conservation.

## Reserve integration evidence

The owner explicitly authorized PR #24 integration on September 8. The merge used expected head `962b84f526c9477ec8c8206dbbf260b33a49426f`.

- Reserve main merge: `4670d4ccedd09f02c2a87a13968e74f1fdfa9f4b`.
- Merge tree: `dcee6ca67d12198c4c73722ee0cbe81ba94ae384`.
- Final PR-head Reserve Conservation Proofs run: `34222683669` — SUCCESS.
- Final PR-head Reserve Verifier Bootstrap run: `34222683630` — SUCCESS.
- Final PR-head Oregon Rust CI run: `34222683676` — SUCCESS.
- Integration evidence: `docs/checkpoints/OREGON_RESERVE_PROOF_MAIN_INTEGRATION.md`.

The reserve integration decision is complete; do not ask for it again.

## Integrated foundation and preserved cautions

- Stage 3B main integration: `fe762f7a5670d94a486423327e3a525cec24afb5`.
- Stage 4A main integration: `dee4ca6ea3b6dc75e4920ab56a44e2b5da8aa0a3`.
- Reserve proof integration: `4670d4ccedd09f02c2a87a13968e74f1fdfa9f4b`.
- Stage 4B and Stage 4C remain separate gated work.
- `FundingCapabilityV1::new` validates data but does not prove a live payer source; future coordination must obtain authority from source validation.
- Existing `WeightMeter` exhaustion remains sticky and charges the maximum; child rollback cannot reset it or cumulative conversion counters.
- No generic unrestricted VM-facing system-domain write or non-revertible write API is permitted.
- Never force-push or move accepted historical checkpoint refs.

Always inspect the real branch, PR and exact-head CI when resuming. This file is a continuation aid, not a substitute for repository state.
