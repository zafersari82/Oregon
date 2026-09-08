# Oregon — current continuation record

Updated: 2026-09-08 UTC.

Stage 3B and Stage 4A remain integrated into `main` as inactive foundations. **Reserve Conservation Proof V1** is integrated through PR #24. The active trust-track work is now **Workstream B — Mutation Evidence Publication V1** on a design branch. No production/runtime semantics are changed by the current documentation slice.

## Resume here

- Repository: `zafersari82/Oregon`.
- Active branch: `design/mutation-evidence-publication-v1-2026-09-08`.
- Branch base: `main` at `5785089e0cc2de18b1b50ef112afbe2f8265a8d8`.
- Written design: `docs/superpowers/specs/2026-09-08-mutation-evidence-publication-v1.md`.
- Design commit after self-review: `66fda8e806500fd3cf81479a3fa332b5994b657e`.
- Trust roadmap: `docs/checkpoints/OREGON_TRUST_VERIFICATION_ROADMAP.md`.
- No Workstream B implementation plan or implementation exists yet.
- No `main` integration decision for Workstream B has been requested or granted.

## Workstream B inventory and approved design direction

The owner approved the proposed Workstream B design direction in chat on September 8. Repository inspection found six existing production mutation authorities invoked by `.github/workflows/oregon-rust.yml`:

- execution address: `3/3` mutations;
- execution envelope: `9/9`;
- contract state: `17/17`;
- execution resources: `13/13`;
- Stage 3B fee settlement/reserve: `14/14`;
- Stage 4A runtime journal: `12/12`.

Total published V1 target inventory: **68 selected mutations across six existing authorities**.

The selected design does not replace those six runners. It adds a fail-closed publication layer containing a versioned manifest, stable mutation IDs, pinned runner SHA-256 identities, schema validation, normalized result records, exact commit/tree binding, clean-checkout/restoration checks, raw evidence retention and exact-head CI evidence.

The design explicitly rejects generic stdout-counting as sufficient evidence and rejects replacing the accepted mutation authorities with a new competing mutation framework.

## Workstream B claim boundary

Mutation evidence is not formal verification. The intended eventual bounded claim is that Oregon publishes reproducible evidence showing the named tests/gates kill the 68 selected V1 fault injections at an exact recorded source.

Do not claim that:

- mutation testing proves absence of bugs;
- the 68 mutations exhaust all possible faults;
- mutation testing is a mathematical proof;
- reserve formal-model negative controls count as production mutation evidence;
- Workstream B activates Stage 4B, Stage 4C or any VM/runtime path.

Existing transaction/block bytes, monetary/reserve rules, storage representation, networking, mempool behavior and activation state remain frozen unless separately approved.

## Written design review boundary

The written design has been committed and self-reviewed. During self-review, the Python test layout was tightened to the repository's established `scripts/test_*.py` convention instead of leaving the test location conditional.

**Next incomplete action:** the owner reviews/accepts the written design file `docs/superpowers/specs/2026-09-08-mutation-evidence-publication-v1.md`. After that approval, use the repository planning discipline to write the detailed implementation plan. Implementation must begin test-first and must preserve all six existing mutation runners as the production authorities.

Do not start Workstream B implementation merely from the earlier conversational design approval; the written-spec review gate is still open. Once the owner approves this committed spec, do not ask for the same design approval again unless the design boundary changes.

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
