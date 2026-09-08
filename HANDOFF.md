# Oregon — current continuation record

Updated: 2026-09-08 UTC.

Reserve Conservation Proof V1 is integrated through PR #24. The active trust-track work is **Workstream B — Mutation Evidence Publication V1**.

## Resume here

- Repository: `zafersari82/Oregon`.
- Approved written design: `docs/superpowers/specs/2026-09-08-mutation-evidence-publication-v1.md`.
- Implementation plan: `docs/superpowers/plans/2026-09-08-mutation-evidence-publication-v1.md`.
- Design/plan branch head before implementation: `ee46836f10084d75dc3100186c8d44c78bcdc025`.
- Implementation branch: `work/mutation-evidence-publication-v1-2026-09-08`.
- The owner approved the written spec and authorized implementation-plan execution on 2026-09-08. Do not ask for that design approval again unless the design boundary changes.
- No Workstream B `main` integration decision has been requested or granted.

## Workstream B target

Preserve the six existing production mutation authorities and publish a fail-closed evidence layer over their existing results:

- execution address: `3/3`;
- execution envelope: `9/9`;
- contract state: `17/17`;
- execution resources: `13/13`;
- Stage 3B fee settlement/reserve: `14/14`;
- Stage 4A runtime journal: `12/12`.

Total V1 inventory: **68 selected mutations across six authorities**.

The publication layer must provide stable mutation IDs, pinned runner SHA-256 identities, manifest/schema validation, exact commit/tree binding, normalized one-to-one kill records, source-restoration/clean-checkout enforcement, deterministic `result-v1.json`, raw evidence retention and exact-head CI.

Mutation testing remains distinct from formal proof. Do not claim absence of bugs, exhaustive mutation coverage or mathematical proof.

## Current TDD state

The first implementation action is RED-first manifest validation. A test-only commit is being published on the implementation branch together with a minimal `Oregon Mutation Evidence` workflow. The expected RED reason is that `scripts/mutation_evidence.py` does not yet exist. After observing that expected failure, implement only the minimal validator/schema surface required to make the manifest tests green.

Local clone execution was attempted but the current execution container could not resolve `github.com`; therefore RED/GREEN execution evidence for this slice is obtained from GitHub Actions on the exact implementation-branch head rather than claimed locally.

## Preserved boundaries

- Existing transaction/block bytes, monetary/reserve rules, storage representation, networking, mempool behavior and activation state remain frozen unless separately approved.
- Existing six mutation runners remain the production mutation authorities; do not replace them with a competing mutation engine.
- Reserve formal-model negative controls do not count toward the 68 production mutations.
- Stage 4B and Stage 4C remain separately gated.
- `main` is updated only after exact-head acceptance and a separate explicit owner integration decision.
- Never force-push or move accepted historical checkpoint refs.

Always inspect the real branch, PR and exact-head CI when resuming. This file is a continuation aid, not a substitute for repository state.
