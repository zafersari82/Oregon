# Oregon Stage 4B Inactive Coordinator Compilation Amendment

**Status:** owner-delegated corrective amendment discovered during exact-head Rust verification

**Date:** 2026-09-10 UTC

**Applies to:**
- `docs/superpowers/specs/2026-09-06-runtime-coordinator-v1-design.md`
- `docs/superpowers/plans/2026-09-06-runtime-coordinator-v1.md`
- `crates/oregon-execution/src/lib.rs`

## 1. Problem discovered by verification

The Stage 4B implementation plan originally required the coordinator to remain private and inactive while also spelling the module declaration as production `mod coordinator;`. The repository independently requires production code to remain warning-clean under `cargo clippy --workspace --all-targets -- -D warnings` and forbids lint suppression.

Exact-head verification demonstrated that these requirements cannot all be satisfied by an uncalled private Rust module. With `mod coordinator;`, rustc correctly reports the inactive, unexported coordinator graph as dead code. A compile-only anonymous-const reference was also tested and did not make that graph live for the `dead_code` lint. Making the module or a synthetic anchor public would create an API solely to satisfy a lint, and suppressing `dead_code` would violate the engineering constitution and agent contract.

This is a design/verification conflict, not evidence that the coordinator should be activated early.

## 2. Corrected inactive boundary

Until a real production caller is introduced by a separately approved integration slice, Stage 4B keeps:

```rust
#[cfg(test)]
mod coordinator;
```

This is deliberate and temporary. The coordinator source remains the authoritative Stage 4B implementation, but it is not part of the normal production library target while there is no legitimate production consumer.

The following alternatives are explicitly rejected:

- `#[allow(dead_code)]`, `#[expect(dead_code)]`, crate-level lint relaxation, or CI-only dead-code suppression;
- a public or `#[doc(hidden)] pub` coordinator/anchor API whose only purpose is liveness;
- `#[used]`/linker-retention anchors or other compiler tricks that do not represent a real Oregon ownership edge;
- calling the coordinator from an unrelated existing public API merely to make rustc consider it reachable;
- activating a VM, universal-envelope path, chainstate publication path, mempool path, RPC path, or consensus execution path before its separately approved integration design.

## 3. Required verification while inactive

The inactive coordinator must still be compiled and exercised by Stage 4B test targets. Stage 4B closure therefore requires:

1. focused `oregon-execution` tests on x86_64 and ARM;
2. the independent runtime/coordinator vectors from the Stage 4B plan;
3. the adversarial deterministic backends from Task 9;
4. the 16 required compiled/killed mutations from Task 10;
5. full workspace regression, format, docs and Clippy gates;
6. an architecture test pinning that the coordinator is test-gated, is not publicly exported, and contains no dead-code suppression.

Passing test builds must not be described as proof that the normal production library currently links an active coordinator. Stage 4B remains inactive.

## 4. Future removal condition

The `#[cfg(test)]` gate must be removed in the same reviewed change that introduces the first legitimate production ownership edge into the coordinator. That future change must:

- identify the real production caller in an approved versioned design;
- keep the coordinator itself non-public unless a reviewed cross-crate API is genuinely required;
- compile the coordinator in the normal library target without lint suppression;
- pass `cargo clippy --locked --workspace --all-targets -- -D warnings` at the exact head;
- preserve the Stage 4B authority boundaries for meter, escrow, journal, accounting and receipts;
- not infer protocol activation merely from production compilation.

## 5. Plan amendment

For Stage 4B Task 3 Step 2, the sentence requiring `mod coordinator;` while no production consumer exists is superseded by this amendment. Read it as:

> Add only the approved `oregon-runtime` dependency. Keep the coordinator private and test-gated while this slice has no legitimate production consumer. Remove the test gate only together with the first separately approved real production ownership edge; never manufacture reachability or suppress dead-code warnings to satisfy compilation.

All other Stage 4B scope, security, determinism, authority and activation constraints remain unchanged.
