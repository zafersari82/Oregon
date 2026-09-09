# Oregon Mutation Evidence Publication V1 — Design

**Status:** owner-approved design direction; written spec awaiting owner review before implementation planning.

**Date:** 2026-09-08

**Base:** `main` at `5785089e0cc2de18b1b50ef112afbe2f8265a8d8`.

**Roadmap:** Workstream B in `docs/checkpoints/OREGON_TRUST_VERIFICATION_ROADMAP.md`.

## 1. Purpose

Oregon already has six independent production mutation authorities that deliberately weaken consensus/runtime-critical rules and require focused tests or vectors to kill those mutants. The missing trust artifact is not another mutation framework. The missing artifact is a versioned, reproducible and machine-readable publication layer that binds those existing authorities to stable mutation identities, exact source, exact reproduction commands and exact CI evidence.

Mutation Evidence Publication V1 therefore turns the existing mutation gates into a public engineering evidence package while preserving their current role as the production mutation authorities.

This design does not claim formal proof. Mutation evidence demonstrates that the selected injected faults are detected by the named tests/gates. Formal reserve-model verification remains a separate assurance claim.

## 2. Existing production mutation authorities

V1 publishes the six mutation gates already invoked by `.github/workflows/oregon-rust.yml`:

| Authority | Runner | Current published cardinality |
| --- | --- | ---: |
| Execution Address | `scripts/verify_execution_address_mutations.py` | 3 |
| Execution Envelope | `scripts/verify_execution_envelope_mutations.py` | 9 |
| Contract State | `scripts/verify_contract_state_mutations.py` | 17 |
| Execution Resources | `scripts/verify_execution_resource_mutations.py` | 13 |
| Fee Settlement / Reserve | `scripts/verify_fee_settlement_mutations.py` | 14 |
| Runtime Journal | `scripts/verify_journal_mutations.py` | 12 |
| **Total** | | **68** |

The existing runners remain authoritative for applying disposable mutations, running their focused Rust/vector checks, rejecting compilation failures as mutation kills where applicable, restoring source and checking clean/restored baselines.

V1 must not silently reimplement their mutation edits in a competing central engine.

## 3. Design choice

### Selected approach: pinned manifest + existing authorities + fail-closed evidence publisher

Add a versioned evidence manifest and a publication/verification layer around the six existing runners.

The manifest describes what Oregon publicly claims. The existing runners continue to decide whether their mutations are actually killed. The evidence publisher verifies that runner identity, mutation inventory and observed results match the manifest before generating any public success artifact.

This separates responsibilities:

1. **Mutation authorities** inject and verify selected faults.
2. **Manifest** defines the public inventory and stable identities.
3. **Publisher** binds the manifest to exact repository source and observed runner evidence.
4. **CI** provides exact-head execution and retained artifacts.

### Rejected alternative: stdout-only wrapper

A thin wrapper that only counts `KILLED:` lines would be easy to add but would not reliably detect runner drift, renamed/deleted mutation definitions, duplicate identities or a changed authority implementation. It is insufficient for a durable public evidence contract.

### Rejected alternative: replace all six runners with one framework

Moving all mutation edits into a new central framework would create a second implementation project, rewrite already accepted authority logic and expand the regression surface without improving the immediate publication goal. V1 instead wraps and verifies existing authorities.

## 4. Repository layout

V1 uses a dedicated verification area:

```text
verification/mutation-evidence/
  manifest-v1.json
  README.md
  schema/
    manifest-v1.schema.json
    result-v1.schema.json
```

Scripts:

```text
scripts/verify_mutation_evidence_manifest.py
scripts/publish_mutation_evidence.py
```

Tests follow the repository's existing Python verification convention:

```text
scripts/test_mutation_evidence_manifest.py
scripts/test_mutation_evidence_publisher.py
```

The data contracts and behavioral requirements in this design are independent of internal helper organization, but the public validator/publisher entrypoints and the two test entrypoints above are fixed for V1.

## 5. Stable mutation identity

Every one of the 68 published mutations receives a stable ID. IDs are namespaced by authority and are never derived from array position at runtime.

Prefixes:

- `EA-` — Execution Address
- `EE-` — Execution Envelope
- `CS-` — Contract State
- `ER-` — Execution Resources
- `FS-` — Fee Settlement / Reserve
- `RJ-` — Runtime Journal

Initial IDs are zero-padded three-digit identifiers, for example `EA-001`, `EE-001`, `CS-001`.

An ID identifies the semantic injected fault, not merely a line number or source offset. If implementation text moves but the same semantic mutation remains, the ID stays. If the semantic fault changes materially, the old ID is retired in a future manifest version and a new ID is allocated; V1 history is not rewritten.

## 6. Manifest V1 contract

`manifest-v1.json` is deterministic JSON and contains at minimum:

- schema/version identifier;
- manifest version;
- authority ID and display name;
- authoritative runner path;
- pinned SHA-256 of the runner file;
- deterministic reproduction command;
- expected mutation count for that authority;
- each mutation's stable ID;
- semantic mutation name;
- target production file/rule area;
- expected broken behavior;
- killing test/vector/gate identifier;
- expected kill classification;
- public claim boundary.

The manifest must contain exactly six authorities and exactly 68 active V1 mutations unless a later separately reviewed manifest version changes the inventory.

The manifest does not duplicate executable replacement strings from the existing runners. This avoids two mutation implementations becoming independent sources of truth. It records semantic identity and binds to the authoritative runner by digest.

## 7. Runner identity and drift protection

Before accepting results, the publisher computes SHA-256 for each configured runner and compares it to the manifest.

Any mismatch is fail-closed and produces no passing publication artifact.

This deliberately means that a legitimate edit to a mutation runner requires review and a manifest update. Public evidence must never quietly refer to a different authority implementation than the one whose inventory was reviewed.

The manifest validator also rejects:

- missing authority;
- unexpected authority;
- duplicate authority ID;
- duplicate mutation ID;
- wrong prefix for an authority;
- missing required mutation field;
- wrong mutation cardinality;
- duplicate semantic mutation entry where uniqueness is required;
- nonexistent runner path;
- invalid runner digest syntax;
- reproduction command that does not point to the bound runner;
- schema/version mismatch.

## 8. Publisher execution model

`publish_mutation_evidence.py` operates only from a clean checkout.

For each authority, in manifest order, it:

1. validates the full manifest before running any mutation authority;
2. records exact Git commit SHA and tree identity;
3. rejects a dirty checkout;
4. verifies runner SHA-256;
5. invokes the existing runner using the manifest command;
6. captures raw combined output, exit status and timing metadata;
7. parses only explicitly supported successful kill/result records;
8. verifies that the observed killed inventory corresponds one-to-one with the manifest authority inventory;
9. verifies the runner's final success summary/cardinality;
10. verifies the checkout is clean after runner completion;
11. stores the authority result in memory only after all checks pass.

The overall publication is successful only when all six authorities pass and all 68 manifest mutations are accounted for exactly once.

No partial set may be emitted as a successful V1 publication.

## 9. Evidence parsing and semantic binding

The publisher must not infer success from process exit code alone or from a generic count alone.

For every observed kill it must bind the runner's semantic mutation name and killing test identifier to the corresponding manifest entry. Where an existing runner uses a slightly different final line syntax, V1 may implement an authority-specific parser adapter, but every adapter must produce the same internal evidence record.

A record equivalent to `KILLED: name — test` or `KILLED: name [test]` is accepted only when both `name` and `test` match the manifest entry for one stable ID.

A final `68/68` aggregate without the individual one-to-one records is insufficient.

## 10. Kill classification and fail-closed rules

A mutation is published as killed only when its existing authority runner itself accepts the intended semantic failure according to that runner's rules and the publisher can bind the accepted record to the manifest.

The publisher must reject success when any of these occur:

- mutation survives;
- mutation site is missing/ambiguous;
- intended test is missing;
- compilation failure is presented instead of the intended test failure;
- timeout or process execution failure;
- runner exits nonzero;
- unexpected or unrelated test failure;
- missing kill record;
- duplicate kill record;
- unknown kill record;
- wrong killing test for a mutation;
- runner summary count disagrees with observed records;
- source is not restored;
- checkout becomes dirty;
- runner digest differs from manifest;
- exact Git source identity cannot be determined.

No fallback mode may convert infrastructure failure into mutation success.

## 11. Machine-readable Result V1

A successful run produces `result-v1.json` conforming to `result-v1.schema.json`.

At minimum it contains:

- result schema/version;
- manifest digest;
- repository identity;
- exact commit SHA;
- exact tree SHA;
- clean-checkout status before and after;
- Rust toolchain command identity expected by the authorities;
- per-authority runner path and verified SHA-256;
- per-authority reproduction command;
- per-mutation stable ID;
- semantic name;
- target rule/file;
- killing test/vector/gate;
- status `killed`;
- authority totals;
- aggregate total `68/68` for V1;
- raw-log artifact filenames/digests when produced in CI;
- CI workflow/run/job identifiers when supplied by the CI environment;
- overall status.

Failed runs may write diagnostic output to the console or a temporary failure file, but a failed run must not leave behind a file that can be mistaken for a passing canonical publication result.

The canonical successful JSON must be deterministic apart from explicitly identified execution-evidence fields such as CI run IDs. Keys and collection ordering are fixed by the generator.

## 12. Human-readable publication

`verification/mutation-evidence/README.md` explains:

- what mutation testing demonstrates;
- why selected semantic mutants provide stronger evidence than line coverage alone;
- why this is still not formal verification;
- how to reproduce all six authorities from a clean checkout;
- how to validate the manifest and result;
- how stable mutation IDs map to target rules and killing tests;
- how to locate exact-head CI artifacts.

A generated or cross-checked technical report may summarize the result, but public prose must be derived from or validated against `manifest-v1.json` and `result-v1.json`. Hand-maintained mutation counts must not become a competing truth source.

## 13. CI design

Add a dedicated mutation-evidence workflow or a clearly isolated job whose purpose is publication evidence rather than replacing `Oregon Rust CI`.

Required properties:

- Linux x86_64 clean checkout;
- read-only repository permissions unless artifact upload requires only the normal Actions token capability;
- Rust 1.85.0 consistent with the existing authorities;
- exact manifest validation before mutation execution;
- all six existing mutation runners executed through the publisher;
- `result-v1.json` generated only after complete success;
- raw per-authority logs retained;
- manifest, result and raw evidence uploaded as one versioned artifact;
- artifact retention sufficient for checkpoint evidence;
- workflow triggers on PRs to `main` and on the implementation branch;
- no path filter that can allow a mutation runner, manifest, publisher, targeted production source or relevant test change to bypass publication verification.

The inherited `Oregon Rust CI` mutation steps remain in place. The publication workflow is additional evidence plumbing, not a replacement for existing integration gates.

ARM mutation execution is not required by V1 because the mutation authorities are semantic Rust/test gates rather than architecture-vector portability claims. Existing independent x86_64/ARM vector evidence remains separately named where relevant.

## 14. Test-first requirements

Implementation begins with parser/manifest/publisher tests that fail for the old nonexistent publication layer for semantic reasons.

At minimum the suite must prove rejection of:

- duplicate mutation ID;
- missing one of the 68 mutations;
- extra unknown mutation;
- wrong authority count;
- modified runner with stale digest;
- malformed digest;
- nonexistent runner;
- fake `KILLED` line for an unknown mutation;
- duplicate `KILLED` line;
- correct mutation name with wrong killing test;
- summary count that disagrees with records;
- nonzero runner exit;
- compile-error text masquerading as a kill where not allowed by the authority contract;
- dirty checkout before execution;
- dirty checkout/source-restoration failure after execution;
- inability to resolve commit/tree identity;
- partial five-authority success;
- stale or malformed result schema.

Positive tests use captured representative outputs from each existing runner syntax and prove deterministic normalization into the Result V1 internal record.

Tests must not invoke all 68 real Rust mutations merely to test parser edge cases. Real full authority execution is a separate integration/CI acceptance step.

## 15. Source identity and reproducibility

A canonical publication result binds to a single exact Git source.

The publisher records both commit SHA and tree SHA. It rejects detached/unresolved repository state only if the exact commit/tree cannot be established; detached HEAD in CI is acceptable when both identities are resolvable and the checkout is clean.

The evidence checkpoint must distinguish:

- implementation source;
- final exact PR-head source;
- actual `main` merge source after a separately authorized integration;
- whether the publication workflow reran on the merge source or whether equivalence is established only by verified tree identity.

A green ancestor never substitutes for final-source acceptance.

## 16. Security and integrity boundaries

The publisher is an evidence tool, not production consensus code.

V1 must not:

- alter production monetary, reserve, envelope, state, execution-resource or journal semantics;
- add mutation code to production builds;
- commit mutated source;
- weaken any current mutation runner's kill criteria;
- count reserve formal-model negative controls as production mutations;
- claim mutation testing proves absence of bugs;
- claim the 68 mutations exhaust all possible faults;
- treat line or branch coverage as equivalent to semantic mutation evidence;
- activate Stage 4B, Stage 4C or any VM/runtime path;
- change transaction/block/storage/network/mempool formats or activation state.

## 17. Versioning

`manifest-v1.json` is immutable as a historical evidence contract once V1 is accepted into `main`, except for corrections performed through an explicitly versioned superseding manifest and checkpoint.

Future additions use a new manifest version when they change the published inventory or schema contract materially. Stable IDs from V1 are not renumbered.

Runner source may evolve, but an accepted publication always records the exact runner digest used for that result.

## 18. Acceptance criteria

Workstream B V1 is accepted only when all of the following are true at the exact final candidate source:

1. manifest and result schemas are committed and validated;
2. exactly six authorities and 68 V1 mutations are represented with stable IDs;
3. all runner digests match the reviewed manifest;
4. manifest/parser/publisher negative tests pass;
5. a clean checkout reproduces all six existing authorities through the publisher;
6. every advertised mutation is killed by its intended gate and appears exactly once in the result;
7. aggregate publication reports exactly `68/68` for V1;
8. source restoration and post-run clean-checkout checks pass;
9. deterministic `result-v1.json` is generated;
10. raw logs plus manifest/result are retained in CI evidence;
11. inherited `Oregon Rust CI` remains green at exact head;
12. a checkpoint records exact source/tree, manifest digest, runner digests, CI run/job/artifact identifiers and claim boundaries;
13. `HANDOFF.md` points to the actual next incomplete action;
14. separate explicit owner approval is obtained before integration into `main`.

## 19. Permitted public claim

After acceptance, the strongest default claim is:

> Oregon publishes reproducible mutation evidence for 68 selected consensus/runtime-critical fault injections across six production mutation authorities; the named tests and gates kill every published V1 mutant at the recorded exact source.

A shorter statement may be used only if it preserves that bounded meaning.

Do not call this a mathematical proof, exhaustive bug proof or proof that all Oregon consensus/runtime behavior is correct.

## 20. Implementation transition

After this design is reviewed, implementation planning should decompose the work into:

1. manifest/schema and fail-closed validator;
2. parser adapters and normalized result model;
3. publisher and Git/source integrity checks;
4. test-first negative/positive fixtures;
5. population of all 68 stable manifest records from the existing authorities;
6. CI evidence workflow and artifact retention;
7. full clean-checkout `68/68` execution;
8. checkpoint, README and `HANDOFF.md` closure;
9. separate `main` integration decision.

No implementation begins merely because this file exists; proceed only after the owner reviews this written design and authorizes the implementation plan transition.
