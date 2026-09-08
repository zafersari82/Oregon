# Oregon — current continuation record

Updated: 2026-09-08 UTC.

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

## Verified implementation and current correction

Source `11b82780c013f7b104b48f7947c891e819563e1a`, tree
`31a28a31a8e418ed18714136a51546293b332a86`, passed:

- Mutation Evidence run `34263606532`, job `102187372191`.
- Rust CI run `34263606628`, job `102187372524`.
- Reserve Conservation Proofs run `34263606596`.
- Reserve Verifier Bootstrap run `34263606616`.

The mutation job's printed canonical result records six authorities, 68/68 killed,
clean source before/after, and the exact source above. Artifact `10071104970`
has ZIP digest `7bdb4730c73b1c9e93430f33b6120a30ed8481d3d7fec36d7a497eaf62916630`.
Its seven-file inventory omits the manifest required by design section 13.
Thus this is verified execution evidence, not complete publication acceptance.

The continuation adds the exact manifest bytes to the output package, with their
SHA-256 bound in `result-v1.json`, and fixes README commands to use script discovery
and suppress bytecode files that would dirty the required clean checkout.
A new regression test failed because the manifest was absent before the correction,
then passed afterward. The original 41 Python tests passed on a clean checkout.
These are local Python checks, not local Rust mutation or Kani execution.

**Next incomplete action:** verify the corrected PR head in live CI, inspect its
artifact for six raw logs plus manifest/result and matching identities, then record
the Workstream B acceptance checkpoint. Keep ancestor execution evidence distinct
from final-head acceptance. PR #25 remains draft; main is unchanged in this
continuation. Historical PRs #22/#23 are superseded reserve work, not the active
next implementation target. Stage 4B remains separate.

## Authorized publication continuation

The owner explicitly authorized publishing the prepared correction commits to
PR #25 with "evet" in response to the specific push request. The earlier automatic
approval blocker is resolved. Git CLI publication still lacks HTTPS credentials;
the authenticated GitHub connector is the publication path for the corrected tree.
Original local correction: `eac65713073ba6deda2e7c6b737747117f5001d3`, tree
`ced53b684b32ba748544716cdeebbb7ea2598a44`. All 42 Python publication tests and
the six-authority/68-mutation manifest validator passed. Connector publication
uses a new commit identity; do not claim the original local commit SHA was pushed.
After publication, verify PR #25's actual head and its live CI/artifact inventory.
No main integration is included in this publication authorization.

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
