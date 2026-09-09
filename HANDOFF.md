# Oregon — current continuation record

Updated: 2026-09-09 UTC.

## Resume here

- Repository: `zafersari82/Oregon`.
- Active branch: `work/mutation-evidence-publication-v1-2026-09-08`, PR #25.
- Main baseline: `5785089e0cc2de18b1b50ef112afbe2f8265a8d8`.
- Reserve Conservation Proof V1 is integrated through PR #24.
- Current work: Workstream B, Mutation Evidence Publication V1.
- Design: `docs/superpowers/specs/2026-09-08-mutation-evidence-publication-v1.md`.
- Plan: `docs/superpowers/plans/2026-09-08-mutation-evidence-publication-v1.md`.

The manifest, schemas, validator, publisher, 68 stable mutation records and dedicated
CI workflow are implemented. Older design/plan approval-status prose predates this
implementation and must not be read as a claim that these files are absent.
The six existing mutation runners remain the production authorities.

## Accepted implementation and closure

Workstream B implementation is verified at
`1de457fa985e0af1c8149a8e8c51ead4e80679a7`, tree
`8f95631380407804bec28bcbf866fb8749989171`.

- Acceptance checkpoint: `docs/checkpoints/OREGON_MUTATION_EVIDENCE_V1.md`.
- Overall platform status: `docs/checkpoints/OREGON_PLATFORM_COMPLETION_STATUS.md`.
- Mutation Evidence run `34273405259`, job `102220420829`: SUCCESS, 68/68.
- Rust CI run `34273405269`, job `102220421102`: SUCCESS.
- Reserve Conservation Proofs run `34273405267`, job `102220421133`: SUCCESS.
- Bootstrap run `34273405305`, job `102220420999`: SUCCESS.
- Artifact `10074897462` retains manifest/result and six raw logs. CI separately
  reread all eight files, checked source/runner/manifest/log identities, and
  reconstructed all 68 semantic records before upload. All 50 Python tests passed.

**Next incomplete action:** publish this documentation closure to PR #25 and
verify its own final-head CI. Record successor CI IDs in the PR body, then obtain
the separate explicit main-integration decision required by plan Task 10.
Earlier push approval remains valid; do not request it again. No integration or
activation is claimed by these documents.

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
