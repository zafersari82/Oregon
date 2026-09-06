# Stage 4A Bounded Runtime Journal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the inactive bounded frame journal with checked snapshot reads and atomic unpublished finalization.

**Architecture:** `oregon-execution` owns frame overlays and lifecycle. It depends downward on `oregon-contract-state` for all checked reads/write-set normalization/SMT transitions. No runtime crate, VM adapter or active block caller is added.

**Tech Stack:** Rust 1.85.0, thiserror, existing proptest/serde_json test dependencies, Python independent BLAKE3 vectors, GitHub Actions x86_64/ARM.

**Spec:** `docs/superpowers/specs/2026-09-06-runtime-journal-async-v1-design.md`, approved for Stage 4A on 2026-09-06.

## Global Constraints

- Base design commit `4b2115aeab3ced67f606240c75e1451f9891a78f`; main `fe762f7a5670d94a486423327e3a525cec24afb5` remains unchanged during this implementation.
- Live depth 64, total frames 4,096, live entries 65,536, retained raw key/value bytes 16,777,216; lower positive validated limits allowed.
- Key/value bounds reuse Stage 2 1,024/1,048,576 bytes; at most six borrowed snapshots, unique and nonempty.
- Only Wasm, ExecutionAccounting, ExecutionReceipts, AsyncOutbox, AsyncConsumed and FeeState; reject NativeUtxo and Evm.
- One root frame; top-frame operations only; child commit stays revertible by ancestors. Total frames never refunded.
- Present-empty is not deletion. Raw-key duplicates collapse only within journal visibility; Stage 2 path collisions remain errors.
- Check limits before copying; count separate retained frame copies; errors leave state unchanged. Finalization consumes the journal and returns no partial domain prefix.
- Existing fees/meter, Stage 2 algorithms, current consensus/state/network/activation paths and existing vectors remain unchanged.
- No TODO/FIXME/HACK, lint suppression or unsafe code. All writes are unpublished proposals, not authorization.

## Task 1: Journal behavior, bounds and finalization

**Files:**
- Create `crates/oregon-execution/src/journal.rs` and private responsibility modules under `src/journal/` if needed.
- Modify `crates/oregon-execution/src/lib.rs`, `crates/oregon-execution/Cargo.toml`, `Cargo.lock`.
- Create `crates/oregon-execution/tests/journal.rs`, `tests/common/mod.rs` only if test fixtures are shared.

**Interfaces:**
- `JournalContextV1 { chain_id: u64, height: u64, parent_block_hash: Hash256, txid: Hash256 }` (local data, not authentication).
- `JournalLimitsV1::new(max_depth: usize, max_frames: usize, max_entries: usize, max_bytes: usize) -> Result<Self, JournalError>` and `Default` selecting hard ceilings.
- `ExecutionJournalV1::new(source: &S, context: JournalContextV1, snapshots: &[DomainSnapshot], limits: JournalLimitsV1) -> Result<Self, JournalError>` where `S: StateSource + ?Sized`.
- `begin_frame`, `commit_frame`, `revert_frame`: `&mut self -> Result<(), JournalError>`.
- `put(&mut self, domain: CommitmentDomainId, key: &[u8], value: &[u8])`, `delete(&mut self, domain, key)` returning `Result<(), JournalError>`.
- `read(&self, domain, key) -> Result<Option<Vec<u8>>, JournalError>`.
- `finalize(self, intent: JournalIntentV1) -> Result<JournalResultV1, JournalError>`; intent variants `Committed`, `Reverted`.
- `JournalResultV1 { context: JournalContextV1, roots: Vec<JournalDomainRootsV1>, transitions: Vec<StateTransition> }`; roots entries `{ domain, old_root, new_root }`, sorted numerically; transitions for changed domains only.
- Typed JournalError separates invalid limits/snapshots/domains, root misuse/open children, depth/frame/entry/byte limits and wrapped Stage 2 errors. No public frame IDs or mutation callbacks.

- [x] Write expected-red behavior tests with a hash-addressed test StateSource implementing both trait methods; populate nonempty sources using Stage 2 transitions. Reuse these fixtures for later vectors.

```rust
let mut journal = ExecutionJournalV1::new(&source, context, &snapshots, JournalLimitsV1::default()).unwrap();
journal.put(domain, b"key", b"parent").unwrap();
journal.begin_frame().unwrap();
journal.put(domain, b"key", b"child").unwrap();
journal.begin_frame().unwrap();
journal.put(domain, b"key", b"grandchild").unwrap();
journal.commit_frame().unwrap();
journal.revert_frame().unwrap();
assert_eq!(journal.read(domain, b"key").unwrap(), Some(b"parent".to_vec()));
```

Add distinguishing tests for every spec section 13 requirement, including exact/one-over limits, repeated sibling churn, old corrupt value on no-op/delete/replacement, and failure in the second domain. Check final roots with existing checked reads, not private journal fields. Add a bounded clone-map reference-model property test for nested traces; it must not reuse production overlay logic.

- [x] Run `cargo +1.85.0 test --locked -p oregon-execution --test journal`. Record missing journal API as expected red before production code. The new downward dependency may be added with the test harness; update lockfile using Cargo. If local toolchain is unavailable, push the test-only head to a draft PR and observe intended CI failure before implementation.
- [x] Implement validated limits/context/errors plus private bounded frame maps. For a write, derive replacement byte delta using borrowed lookups, validate new totals, then copy/insert. For child merge, validate accounting before removal, move owned entries, subtract replaced parent entries. Revert removes only top-frame retained usage, never frame-created history.

```rust
// Final commitment authority remains in oregon-contract-state.
let writes = StateWriteSet::new(snapshot.domain, final_writes)?;
let transition = apply_write_set(source, snapshot, &writes)?;
// Keep every transition private until all domains have succeeded.
```

- [x] Run journal tests, `cargo +1.85.0 test --locked -p oregon-execution --all-targets`, format and warnings-denied execution Clippy. Record output and commit tests/code together after preserving the red evidence. Run spec/quality review before Task 2.

## Task 2: Independent vectors and security gates

**Files:**
- Create `scripts/generate_journal_vectors.py`, `tests/vectors/journal-v1.json`, `crates/oregon-execution/tests/journal_vectors.rs`.
- Create `scripts/verify_journal_mutations.py`, `.github/workflows/oregon-journal.yml`.
- Modify `.github/workflows/oregon-rust.yml`; add focused journal/vector/mutation commands and require design/plan presence, preserve all inherited gates.

**Interfaces:** Task 1 public API above; literal JSON corpus with named cases, base key/value state, operations, final committed/reverted state and expected domain roots. Hex bytes are canonical lowercase; domain ids numeric. Python computes roots independently, never invokes Rust; Rust reads committed JSON only.

- [x] Create test/vector consumers covering nonempty base, nested child success/revert, ancestor revert, deletion/present-empty, multiple domains and whole-transaction revert. Reuse established BLAKE3 domain framing from the existing independent generator; use an independent bottom-up sparse tree oracle rather than porting Rust.

```python
def domain_hash(domain, payload):
    return blake3(domain + payload).digest()
# --check compares regenerated JSON bytes with the committed corpus.
```

- [x] Run oracle `--check` and Rust vector consumers on x86_64 and ARM. Ensure the vector workload exercises journal trace execution, not only the Stage 2 root helper. A deliberately changed expected root must fail before restoring the literal corpus.
- [x] Implement unique-site mutation replacements for `child_revert_leaks`, `ancestor_revert_keeps_child`, `delete_falls_through`, `empty_value_becomes_absent`, `wrong_domain_allowed`, `frame_depth_bypass`, `frame_count_refunded`, `retained_bytes_undercounted`, `entry_limit_bypass`, `open_child_finalize_allowed`, `partial_domain_result_published`, `unchecked_base_read`. Use intended named test failures, reject compile failures, restore original source bytes in finally and hash-verify restoration after every mutation.

```bash
python3 scripts/generate_journal_vectors.py --check
cargo +1.85.0 test --locked -p oregon-execution --test journal_vectors
python3 scripts/verify_journal_mutations.py
```

- [x] Add pinned-checkout CI matching existing workflows, matrix `[ubuntu-24.04, ubuntu-24.04-arm]`, Rust 1.85.0. Run Python check plus journal/vector tests in both matrix jobs. Add the mutation gate before rustdoc/docs/format/Clippy in full Rust CI.
- [x] Commit and request scoped spec/quality review, then broad final review. Address load-bearing findings with covering tests and re-review.

## Task 3: Exact-head verification and persistent handoff

**Files:** `docs/checkpoints/OREGON_RUNTIME_JOURNAL_PROGRESS.md`, `HANDOFF.md`, this plan.

- [x] Publish implementation as a draft PR against main, retaining approved design ancestry. Record exact SHA/tree and links; do not merge PR #18 or implementation without separate integration approval.
- [x] Require full workspace/all-target tests, architecture scan, inherited mutation gates, new 12/12 journal gate, x86_64/ARM journal vectors, rustdoc/docs, format and warnings-denied Clippy on the exact implementation head.
- [x] Record local/native-toolchain limitations precisely. Local focused success is not full-workspace evidence.
- [x] Write checkpoint naming approved scope, public APIs, mutation and vector counts, exact CI IDs, inactive limitations and next action (Stage 4B design after separate integration decision). Preserve historical records.
- [ ] Commit checkpoint/HANDOFF and verify that documentation successor's exact-head CI, then update PR body with closure evidence. Stop before main integration. Do not report VM, fee coordinator, async execution or durable execution acceptance as delivered by 4A.

Status note: every implementation-head item above is satisfied at `f15ac4db43801fed3dad3d9fb03268e7d34cf43a`. The final checkbox intentionally remains open until the checkpoint/HANDOFF successor commit created from this plan has completed its own exact-head CI and PR #19 is updated with that closure evidence.
