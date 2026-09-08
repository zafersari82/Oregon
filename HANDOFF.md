# Oregon — current continuation record

Updated: 2026-09-08 UTC.

Stage 3B and Stage 4A remain integrated into `main` as inactive foundations. The current active trust-track work is **Reserve Conservation Proof V1** on the isolated branch `work/reserve-proof-arithmetic-2026-09-08`, PR #24.

## Resume here

- Repository: `zafersari82/Oregon`.
- Active branch: `work/reserve-proof-arithmetic-2026-09-08`.
- Active PR: #24.
- PR base before closure: `main` at `a530ae1d20e5a1648cf9cfea045221ec13326d25`.
- Owner-approved design: `docs/superpowers/specs/2026-09-06-reserve-conservation-proof-v1.md`.
- Plan: `docs/superpowers/plans/2026-09-06-reserve-conservation-proof-v1.md`.
- Acceptance checkpoint: `docs/checkpoints/OREGON_RESERVE_CONSERVATION_PROOF_V1.md`.
- The owner-approved proof design must not be requested again unless the design boundary changes.

## Accepted proof implementation

The proof/model/control implementation was accepted at exact source:

- SHA `511f61302ee486e618a33235808572ae4419f487`.
- Tree `e2f12792a08a5376807baa4e92b4ca63ef44a56b`.

At that source:

- Oregon Reserve Conservation Proofs run `34220257918`, job `102041448968`: **SUCCESS**.
- Oregon Reserve Verifier Bootstrap run `34220257907`, job `102041448925`: **SUCCESS**.
- Oregon Rust CI run `34220257858`, job `102041448416`: **SUCCESS**.
- Proof artifact ID `10053933077`, digest `sha256:e84b33e5736ff9597baf6f0a2521bec3082ad0ca97da1188f4c1e6f35d303953`.

The proof CI verifies pinned Kani 0.67.0 / CBMC 6.8.0 / CaDiCaL 2.0.0 identities before execution. All RC01–RC10 positive proof harnesses pass. All ten required disposable-model negative controls are semantically killed with concrete counterexamples, baseline source restoration is checked, and the complete positive RC01–RC10 suite is rerun afterward.

The state proof harnesses use explicit unwind bound `5`. Kani proof execution is Linux x86_64 only; do not claim ARM Kani verification.

## Production correspondence

`oregon-utxo` compiles the exact verification model source through:

`#[path = "../../../verification/reserve-conservation/src/model.rs"] mod reserve_model;`

Exact-head Rust CI runs the production/model correspondence tests and passed constructor error precedence, overflow/underflow and endpoint cases, zero/positive cardinality, exact reserve metadata/program, multiple-reserve rejection, collision atomicity, tampered-undo rejection and full apply/undo restoration.

The RC07 model equality issue found during formal verification was resolved in the verification model by comparing bounded state as map content rather than slot position, matching production `HashMap` state semantics. Production reserve behavior was not changed by this proof slice.

Inherited production mutation authorities remained green at the accepted source:

- execution address `3/3`;
- execution envelope `9/9`;
- contract state `17/17`;
- execution resource `13/13`;
- Stage 3B fee settlement/reserve `14/14`;
- Stage 4A journal `12/12`.

Accepted Stage 3B independent fee-settlement/reserve vectors remain successful on x86_64 and ARM (run `34039255396`, jobs `101502890461` and `101502890592`). This is retained executable production-vector evidence for unchanged Stage 3B code, not ARM Kani proof evidence.

## Exact proof claim and limitations

Permitted claim:

> Kani verifies the listed reserve-conservation properties of Oregon's bounded reserve model; differential tests check correspondence with the inactive Stage 3B implementation.

Do not broaden this into proof of all production Rust, arbitrary/unbounded UTXO state, storage/WAL/crash behavior, block integration, cross-domain reorganization, VM execution, authorization/account-total provenance, or global native OREG supply conservation. Full limitations are recorded in the acceptance checkpoint.

## Closure state and next incomplete action

After accepted proof source `511f6130...`, the branch is receiving documentation-only / CI-label closure commits:

- CI mutation step renamed to accurately state RC01–RC10.
- Reserve proof acceptance checkpoint added.
- Implementation plan completion state updated.
- This `HANDOFF.md` updated to current evidence.

**Next incomplete action:** verify the exact final closure head with Reserve Conservation Proofs, Reserve Verifier Bootstrap and Oregon Rust CI. If all applicable exact-head gates are green and PR #24 remains clean/mergeable, stop at the integration boundary unless the owner has made the repository-required separate explicit decision to integrate into `main`.

After an authorized merge, verify the actual `main` merge source with its triggered CI before claiming main integration complete. Then continue the trust roadmap with Workstream B: reproducible mutation-evidence publication.

## Integrated foundation and preserved cautions

- Stage 3B main integration: `fe762f7a5670d94a486423327e3a525cec24afb5`.
- Stage 4A main integration: `dee4ca6ea3b6dc75e4920ab56a44e2b5da8aa0a3`.
- Trust roadmap: `docs/checkpoints/OREGON_TRUST_VERIFICATION_ROADMAP.md`.
- Stage 4B and Stage 4C remain separate gated work; reserve-proof acceptance is not runtime activation.
- Existing transaction/block bytes, monetary rules, storage representation, networking and mempool behavior remain frozen unless separately approved.
- `FundingCapabilityV1::new` validates data but does not prove a live payer source; future coordination must obtain authority from source validation.
- Existing `WeightMeter` exhaustion remains sticky and charges the maximum; child rollback cannot reset it or cumulative conversion counters.
- No generic unrestricted VM-facing system-domain write or non-revertible write API is permitted.
- Never force-push or move accepted historical checkpoint refs.

Always inspect the real branch, PR and exact-head CI when resuming. This file is a continuation aid, not a substitute for repository state.
