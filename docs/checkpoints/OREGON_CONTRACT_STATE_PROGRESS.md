# Oregon Contract State Progress Checkpoint

Recorded: 2026-09-06 UTC. Implementation verified: 2026-09-05 UTC.

Status: **verified inactive Stage 2 implementation**. This is not execution activation or main-integration approval.

## Authority and immutable sources

- Accepted main: `bf7675bfe17182f77d4c43e2bcbd0c283709d799`.
- Execution Architecture V1: `ed67ccb89131970571d93911cf5553be33636e2f`, PR #9.
- Stage 1 envelope/auth head: `b97f9d3af9e2c9c4011750cfb69cce8fd9117a8a`, PR #12.
- Stage 2 design/plan head: `8a3f2c51f7c4ed7078fa808b2223f3b6af4ef3a7`, PR #13.
- Implementation branch: `work/contract-state-v1-2026-09-05`, draft PR #14.
- Verified implementation: `6bfc3364c71674c2bc3dd20e2b66a4fcabaac19f`.
- Verified tree: `682a1fc617dad937efbaa3b24ae5c7d861674ad6`.
- Spec: `docs/superpowers/specs/2026-09-05-contract-state-commitments-v1.md`.
- Plan: `docs/superpowers/plans/2026-09-05-contract-state-commitments-v1.md`.

## Delivered scope

`oregon-primitives` owns closed commitment ids, exact 36-byte child descriptors, canonical ordered aggregate sets and aggregate hashing. Literal vectors cover every known domain and both known schemes; the EVM scheme remains reserved.

`oregon-contract-state` owns the storage-independent 256-bit MSB-first SMT: domain-separated hashes, empty ladders, immutable nodes, canonical bounded write sets, deterministic batch transitions, snapshots, checked reads, and compressed membership/non-membership proofs. Read, proof and update paths reuse the checked source boundary. Path-bit interpretation has one implementation.

Transitions return unpublished logical records. Retained roots remain readable after later updates; valid no-op writes produce no new records. Present empty values remain distinct from absence. Normalization types share `transition.rs`; there is no redundant write-set module or alternative execution path.

Current header bytes, block ids, transaction encoding/txids, RocksDB schema, chainstate, native UTXO, monetary rules, mempool, networking and node behavior are unchanged against accepted main. RPC, wallet, EVM/WASM runtimes, persistence integration and execution activation remain later work.

## Test-first evidence and audit repairs

Original primitive expected-red source: `70f6e8dba8a42a77a592840797761d07dcc7cf63`, CI `33983342052`, job `101352373000`: missing state-commitment API (E0432).

Original SMT expected-red source: `e9bf8986b6a0f0173206ae9fa9968193d26da6c3`, CI `33983809286`, job `101353609780`: missing hash APIs (E0432). Transition/proof test-first commits remain in branch history.

Continuation audited `d3e89ab13bd70604b14b82893519f54664f594ba`. Its CI `33985620545` passed tests/mutations/docs but failed format and skipped Clippy; it was not a completed checkpoint. Independent review found two correctness gaps:

1. Deletion, replacement and same-value no-op did not validate the old referenced value blob.
2. A proof decoded under one domain could contain an explicit default sibling for another verification domain.

Regression source `e501bf6e53c4671944817165e3794c9ce3553a2e` reproduced six missing/corrupt-old-value failures and one proof-canonicality failure locally under Rust 1.85.0. CI `33986591464`, job `101361170925`, independently failed the intended `verification_rejects_default_sibling_for_its_own_domain` test after inherited/primitive gates passed. Cargo stopped at the proof binary; the six old-value expected-red results were local.

The fixes validate the old value before any update/no-op decision and reject explicit default siblings for the verification domain. Scoped independent review confirmed both repairs and the required coverage with no remaining blocking finding. Inherited format/Clippy issues were corrected without suppression.

## Verification

The exact implementation commit above passed:

| Gate | Evidence |
| --- | --- |
| Oregon Rust CI | Run `33990488016`, job `101371722660`: SUCCESS |
| Architecture and focused inherited/new contracts | SUCCESS |
| Full workspace, all-target tests | SUCCESS |
| Address mutations | 3/3 killed |
| Envelope mutations | 9/9 killed |
| Contract-state mutations | 17/17 killed; restored clean suites passed |
| Chainstate rustdoc and workspace docs | SUCCESS |
| Rustfmt and workspace Clippy with warnings denied | SUCCESS |
| RandomX architecture vector | Run `33990488002`: x86 and ARM SUCCESS |
| RandomX full/light parity | Run `33990487993`: x86 and ARM SUCCESS |

Local focused verification passed 39 contract-state tests and 5 primitive commitment tests, format and Clippy. An isolated local checkout also killed all 17 mutants and passed the restored suites. An earlier run in the active worktree stopped on its restoration guard; that run is not acceptance evidence and its remaining mutant was restored from the committed source before continuing.

Coverage includes exact/max-plus-one key, value, write-count and proof ceilings; hostile proof bytes through 8,500 bytes; random small writes; malformed/tampered and wrong-context proofs; corruption; no-op outputs; and reads from retained snapshots. The nonempty-tree non-membership literal is both constructed and verified.

Additional accounting 16-bit shared-prefix and all-domain aggregate vectors were generated independently with Python BLAKE3 1.0.9 and a bottom-up map of integer path prefixes, after cross-checking existing literals. Rust tests compare literal expectations rather than deriving them from production code.

## Mutation coverage

All ten spec §24 requirements are represented: missing domain binding, LSB-first paths, omitted branch depth, present-empty becoming absent, duplicate paths, missing-node substitution, redundant explicit default siblings, omitted aggregate scheme, noncanonical aggregate ordering/duplicates, and ignored proof domain.

Seven additional mutants cover old-value verification, verification-domain canonicality, a zero empty-leaf hash, failed subtree collapse, omitted aggregate domain, bitmap/count mismatch, and proof construction retaining defaults.

The runner requires a clean disposable checkout, verifies unique mutation sites, accepts only the intended named test failure, rejects compilation failure as evidence, restores in `finally`, and checks restoration after every mutant and the final baseline.

## Metadata successor and next work

The checkpoint successor removes only the two temporary RandomX push-branch entries and updates this checkpoint, the plan and handoff. Code, tests and vectors remain identical to the verified implementation. That clean descendant requires its own Oregon Rust CI; consult current branch HEAD and PR #14 for its exact commit/run before editing.

Stage 2 is complete as an inactive logical-state library. Next is a versioned Stage 3 design for normalized resource weight, fee escrow/settlement, fee-state transition and UTXO reserve conservation under Execution Architecture V1 §27. Activation parameters need the specified benchmark/vector evidence. Do not repeat Stage 1/2 or silently activate execution.

Main integration, persistence/GC, runtime/VM integration and execution activation remain separate later work.
