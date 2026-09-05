# Contract State and Commitments V1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the inactive Oregon Stage 2 logical contract-state commitment foundation: typed child commitments, aggregate root, a deterministic 256-bit Sparse Merkle Tree, immutable snapshots/transitions, canonical write sets, and compressed membership/non-membership proofs.

**Architecture:** `oregon-primitives` owns byte-exact commitment descriptors and aggregate-root identity. A new storage-independent `oregon-contract-state` crate owns Oregon Sparse Merkle V1 logical semantics using immutable content-addressed nodes, batch application of canonical path-sorted writes, and storage-neutral proof/read interfaces. Existing block/header/transaction/storage/chainstate activation paths remain untouched.

**Tech Stack:** Rust 1.85.0, edition 2024, existing BLAKE3/`Hash256`, `thiserror` 2, `serde`/`serde_json` for literal test vectors, `proptest` for hostile-input tests, GitHub Actions as authoritative verification.

**Spec:** `docs/superpowers/specs/2026-09-05-contract-state-commitments-v1.md`

## Global Constraints

- Base/source: clean Stage 1 branch head `b97f9d3af9e2c9c4011750cfb69cce8fd9117a8a`.
- Design branch: `design/contract-state-commitments-v1-2026-09-05`.
- Do not modify current 114-byte `BlockHeader`, block-id rules or `Transaction::encode/decode/txid`.
- Do not add RocksDB schema/column families or make `oregon-contract-state` depend on `oregon-storage`.
- Domain ids: native UTXO `0x0001`, EVM `0x0010`, WASM `0x0011`, execution accounting `0x0020`, receipts `0x0030`, async outbox `0x0040`, async consumed `0x0041`, fee state `0x0050`.
- Scheme ids: `OREGON_SMT_V1 = 0x0001`, reserved inactive `EVM_COMMITMENT_V1 = 0x0100`.
- Descriptor is exactly `u16 domain LE || u16 scheme LE || 32-byte root`.
- Aggregate version is `1`; one to 32 descriptors; strict ascending domain id; duplicate domains invalid.
- Aggregate domain hash: `OREGON/STATE/AGGREGATE/V1\0`.
- SMT is fixed-depth 256; root depth `0`; leaf depth `256`; path bits MSB-first.
- Hash domains: `OREGON/STATE/SMT/KEY/V1\0`, `.../VALUE/V1\0`, `.../EMPTY/V1\0`, `.../LEAF/V1\0`, `.../NODE/V1\0` exactly as frozen in the spec.
- Internal-node hash commits `domain_id`, `depth`, `left`, `right`.
- `Some([])` is present empty value; `None` is absence/deletion.
- Key ceiling 1,024 bytes; value ceiling 1,048,576 bytes; write-set ceiling 65,536 entries; proof max 8,226 bytes.
- Missing/corrupt referenced non-empty nodes/values fail closed and are never substituted with empty state.
- New nodes/value blobs are immutable/content-addressed; rollback is root selection, not inverse mutation.
- EVM state is not computed by Oregon SMT V1.
- No production `unsafe` and no new external dependencies beyond existing crate families.

---

## File Structure

### `oregon-primitives`

- Create `crates/oregon-primitives/src/state_commitment.rs` — ids, descriptor codec, aggregate set/root and errors.
- Modify `crates/oregon-primitives/src/hash.rs` — expose the existing canonical `domain_hash` helper publicly without changing behavior.
- Modify `crates/oregon-primitives/src/lib.rs` — export commitment primitives and `domain_hash`.
- Create `crates/oregon-primitives/tests/state_commitments.rs` — exact codec/aggregate/adversarial contracts.
- Create `tests/vectors/state-commitments-v1.json` — literal descriptor and aggregate bytes/hashes.

### `oregon-contract-state`

- Create `crates/oregon-contract-state/Cargo.toml` — depends only on `oregon-primitives` and `thiserror`; dev deps `serde`, `serde_json`, `proptest`.
- Create `crates/oregon-contract-state/src/lib.rs` — public API exports only.
- Create `crates/oregon-contract-state/src/error.rs` — typed fail-closed state errors.
- Create `crates/oregon-contract-state/src/hash.rs` — path/value/empty/leaf/branch hash functions and MSB path-bit logic.
- Create `crates/oregon-contract-state/src/node.rs` — immutable `StateNode` and hash validation.
- Create `crates/oregon-contract-state/src/source.rs` — storage-neutral `StateSource` trait.
- Create `crates/oregon-contract-state/src/write_set.rs` — canonical bounded writes and path sorting.
- Create `crates/oregon-contract-state/src/transition.rs` — snapshots, deterministic batch tree update, read API and transition outputs.
- Create `crates/oregon-contract-state/src/proof.rs` — canonical compressed proof codec, construction and verification.
- Create `crates/oregon-contract-state/tests/smt_vectors.rs` — literal root/update/delete vectors.
- Create `crates/oregon-contract-state/tests/proofs.rs` — membership/non-membership vectors and codec contracts.
- Create `crates/oregon-contract-state/tests/security.rs` — corruption, bounds, domain separation, ordering and hostile-input tests.
- Create `tests/vectors/contract-state-smt-v1.json` — literal independent SMT/proof vectors.

### CI/checkpoint

- Modify root `Cargo.toml` — add the new workspace member only.
- Modify `.github/workflows/oregon-rust.yml` — focused commitment/SMT gates before full workspace and mutation gate after full tests.
- Create `scripts/verify_contract_state_mutations.py` — targeted mutation runner.
- Create `docs/checkpoints/OREGON_CONTRACT_STATE_PROGRESS.md` after exact-head green.
- Modify `HANDOFF.md` after exact-head green.

---

### Task 1: Test-first commitment descriptor and aggregate contract

**Files:**
- Create: `crates/oregon-primitives/tests/state_commitments.rs`
- Create: `tests/vectors/state-commitments-v1.json`
- Modify: `.github/workflows/oregon-rust.yml`

**Consumes:** existing `Hash256`, `Decoder`, little-endian integer rules and BLAKE3 domain hashing.

**Expected API from Task 2:**

```rust
use oregon_primitives::state_commitment::{
    CommitmentDomainId, CommitmentSchemeId, StateCommitmentDescriptor,
    StateCommitmentError, StateCommitmentSetV1, MAX_STATE_COMMITMENTS,
};
```

- [ ] **Step 1: Add literal descriptor/aggregate vector file before implementation.** Store canonical hex for at least one WASM descriptor, one execution-accounting descriptor, a two-descriptor set, and independent expected aggregate-root hex. Expected values must be produced independently from the production implementation and then copied literally into JSON.

JSON schema:

```json
{
  "aggregate_version": 1,
  "vectors": [
    {
      "name": "wasm_only",
      "descriptors": [
        {"domain_id": 17, "scheme_id": 1, "root_hex": "<64 lowercase hex>"}
      ],
      "canonical_hex": "<literal hex>",
      "aggregate_root_hex": "<64 lowercase hex>"
    }
  ]
}
```

The angle-bracket notation above describes the JSON field format; the committed vector file must contain concrete lowercase hex and no placeholder tokens.

- [ ] **Step 2: Write failing discriminant tests.** Pin every domain/scheme numeric value and reject unsupported ids with `TryFrom<u16>`.

Representative test contract:

```rust
#[test]
fn commitment_ids_are_frozen_and_unknown_ids_fail_closed() {
    assert_eq!(u16::from(CommitmentDomainId::Wasm), 0x0011);
    assert_eq!(u16::from(CommitmentSchemeId::OregonSmtV1), 0x0001);
    assert!(CommitmentDomainId::try_from(0xffff).is_err());
    assert!(CommitmentSchemeId::try_from(0xffff).is_err());
}
```

- [ ] **Step 3: Write failing descriptor codec tests.** Require exact 36-byte encoding, truncation rejection at every byte boundary, unknown ids rejection, and trailing-byte rejection.

- [ ] **Step 4: Write failing aggregate canonicality tests.** Require one to 32 descriptors, strict domain ordering, duplicate-domain rejection, malformed `(len - 2) % 36 != 0` rejection, version exactly `1`, and literal vector roundtrip.

- [ ] **Step 5: Write failing aggregate identity tests.** Mutate only domain id, scheme id, or child root and require a different aggregate root. Reverse descriptor order and require constructor/decode rejection rather than silent sorting at decode time.

- [ ] **Step 6: Wire a focused `State commitment contracts` CI step** running:

```bash
cargo +1.85.0 test --locked -p oregon-primitives --test state_commitments
```

- [ ] **Step 7: Publish expected-red commit.** Expected failure is unresolved `oregon_primitives::state_commitment`; inherited execution-address/envelope focused tests must remain green. Record exact SHA/run/job.

---

### Task 2: Implement commitment primitives and aggregate root

**Files:**
- Create: `crates/oregon-primitives/src/state_commitment.rs`
- Modify: `crates/oregon-primitives/src/hash.rs`
- Modify: `crates/oregon-primitives/src/lib.rs`

**Produces exact public API:**

```rust
pub const MAX_STATE_COMMITMENTS: usize = 32;

#[repr(u16)]
pub enum CommitmentDomainId {
    NativeUtxo = 0x0001,
    Evm = 0x0010,
    Wasm = 0x0011,
    ExecutionAccounting = 0x0020,
    ExecutionReceipts = 0x0030,
    AsyncOutbox = 0x0040,
    AsyncConsumed = 0x0041,
    FeeState = 0x0050,
}

#[repr(u16)]
pub enum CommitmentSchemeId {
    OregonSmtV1 = 0x0001,
    EvmCommitmentV1 = 0x0100,
}

pub struct StateCommitmentDescriptor { /* private fields */ }
pub struct StateCommitmentSetV1 { /* private descriptors */ }
```

Required methods:

```rust
impl StateCommitmentDescriptor {
    pub fn new(domain_id: CommitmentDomainId, scheme_id: CommitmentSchemeId, root: Hash256) -> Self;
    pub fn domain_id(&self) -> CommitmentDomainId;
    pub fn scheme_id(&self) -> CommitmentSchemeId;
    pub fn root(&self) -> Hash256;
    pub fn encode(&self) -> [u8; 36];
    pub fn decode(bytes: &[u8]) -> Result<Self, StateCommitmentError>;
}

impl StateCommitmentSetV1 {
    pub fn new(descriptors: Vec<StateCommitmentDescriptor>) -> Result<Self, StateCommitmentError>;
    pub fn descriptors(&self) -> &[StateCommitmentDescriptor];
    pub fn encode(&self) -> Vec<u8>;
    pub fn decode(bytes: &[u8]) -> Result<Self, StateCommitmentError>;
    pub fn root(&self) -> Hash256;
}
```

- [ ] **Step 1: Expose existing canonical hash helper, unchanged.** Change only visibility from `pub(crate)` to `pub` and re-export it from `lib.rs`:

```rust
pub fn domain_hash(domain: &[u8], payload: &[u8]) -> Hash256 { /* existing body unchanged */ }
```

- [ ] **Step 2: Implement closed ids and conversions.** `From<enum> for u16` and `TryFrom<u16>` must use explicit match arms; unsupported values return typed `UnknownCommitmentDomain(u16)` / `UnknownCommitmentScheme(u16)`.

- [ ] **Step 3: Implement descriptor codec.** Encode `domain LE || scheme LE || root`; decode exactly 36 bytes using `Decoder`; require `finish()`.

- [ ] **Step 4: Implement aggregate constructor validation.** Reject empty, >32, non-strict domain order and duplicate domain. `new` does not silently sort; callers must supply canonical order.

- [ ] **Step 5: Implement aggregate codec/root.** Encoding is `1u16 LE` plus descriptors with no count. Root is:

```rust
const AGGREGATE_DOMAIN: &[u8] = b"OREGON/STATE/AGGREGATE/V1\0";
domain_hash(AGGREGATE_DOMAIN, &self.encode())
```

- [ ] **Step 6: Run focused green and all `oregon-primitives` tests.**

```bash
cargo +1.85.0 test --locked -p oregon-primitives --test state_commitments
cargo +1.85.0 test --locked -p oregon-primitives --all-targets
```

- [ ] **Step 7: Commit the primitive implementation** without touching `block.rs` or `transaction.rs`.

---

### Task 3: Test-first `oregon-contract-state` crate and SMT vectors

**Files:**
- Modify: root `Cargo.toml`
- Create: `crates/oregon-contract-state/Cargo.toml`
- Create: `crates/oregon-contract-state/src/lib.rs`
- Create: `crates/oregon-contract-state/tests/smt_vectors.rs`
- Create: `tests/vectors/contract-state-smt-v1.json`
- Modify: `.github/workflows/oregon-rust.yml`

**Cargo manifest:**

```toml
[package]
name = "oregon-contract-state"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true

[dependencies]
oregon-primitives = { path = "../oregon-primitives" }
thiserror = "2"

[dev-dependencies]
proptest = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 1: Add only the crate shell and workspace member.** `lib.rs` contains `#![forbid(unsafe_code)]` but none of the expected SMT modules yet.

- [ ] **Step 2: Add literal independent SMT vector JSON before implementation.** Include at minimum WASM and execution-accounting domains with concrete key/path/value/empty-root/one-leaf/two-leaf/update/delete hashes and concrete proof bytes. No runtime generation of expected values.

Required JSON fields per case include concrete lowercase hex for:

```json
{
  "domain_id": 17,
  "key_hex": "...",
  "path_key_hex": "...",
  "value_hex": "...",
  "value_hash_hex": "...",
  "root_hex": "..."
}
```

- [ ] **Step 3: Write failing SMT hash/root contracts** importing the APIs defined in Task 4. Pin MSB-first path behavior including bit 0, bit 7, bit 8 and bit 255.

- [ ] **Step 4: Pin `Some([])` versus absence.** Require empty byte value to create a populated leaf/root different from the domain empty root.

- [ ] **Step 5: Pin cross-domain separation.** Identical raw key/value under WASM and execution-accounting ids must produce different path keys, value hashes and roots.

- [ ] **Step 6: Add focused CI step:**

```bash
cargo +1.85.0 test --locked -p oregon-contract-state --test smt_vectors
```

- [ ] **Step 7: Publish expected-red checkpoint.** Expected failure is unresolved contract-state hash/node API, not Cargo/workspace configuration failure. Record exact SHA/run/job.

---

### Task 4: Implement SMT hashing, empty ladder and immutable nodes

**Files:**
- Create: `crates/oregon-contract-state/src/error.rs`
- Create: `crates/oregon-contract-state/src/hash.rs`
- Create: `crates/oregon-contract-state/src/node.rs`
- Modify: `crates/oregon-contract-state/src/lib.rs`

**Produces exact API:**

```rust
pub const SMT_DEPTH: usize = 256;
pub const MAX_STATE_KEY_BYTES: usize = 1_024;
pub const MAX_STATE_VALUE_BYTES: usize = 1_048_576;

pub fn path_key(domain: CommitmentDomainId, key: &[u8]) -> Result<Hash256, StateError>;
pub fn value_hash(domain: CommitmentDomainId, value: &[u8]) -> Result<Hash256, StateError>;
pub fn empty_hashes(domain: CommitmentDomainId) -> [Hash256; 257];
pub fn leaf_hash(domain: CommitmentDomainId, path_key: Hash256, value_hash: Hash256) -> Hash256;
pub fn branch_hash(domain: CommitmentDomainId, depth: u16, left: Hash256, right: Hash256) -> Result<Hash256, StateError>;
pub fn path_bit(path_key: Hash256, depth: usize) -> Result<bool, StateError>;

pub enum StateNode {
    Leaf { path_key: Hash256, value_hash: Hash256 },
    Branch { depth: u16, left: Hash256, right: Hash256 },
}

impl StateNode {
    pub fn hash(&self, domain: CommitmentDomainId) -> Result<Hash256, StateError>;
}
```

- [ ] **Step 1: Implement typed errors** including `KeyTooLarge`, `ValueTooLarge`, `DepthOutOfRange`, `NonCanonicalEmptyBranch`, `MissingNode`, `NodeHashMismatch`, `NodeDepthMismatch`, `UnexpectedLeaf`, `UnexpectedBranch`, `MissingValue`, `ValueHashMismatch`, `DuplicatePath`, `WriteSetTooLarge`, `ProofTooLarge`, `MalformedProof`, `RedundantDefaultSibling`, `InvalidProof`, `DomainMismatch` and wrapped primitive errors where needed.

- [ ] **Step 2: Implement path/value hashes exactly.** Payload starts with `u16::from(domain).to_le_bytes()` and then canonical bytes.

- [ ] **Step 3: Implement MSB-first path bit.** Exact logic:

```rust
let byte = path_key.as_bytes()[depth / 8];
Ok((byte & (0x80 >> (depth % 8))) != 0)
```

Reject `depth >= 256`.

- [ ] **Step 4: Implement the 257-entry empty ladder.** Compute `empty[256]`, then loop `(0..256).rev()` and include `depth as u16` in every branch hash.

- [ ] **Step 5: Implement immutable leaf/branch hashes.** Reject branch depth >255 and explicit both-default branch construction in the canonical node constructor/helper.

- [ ] **Step 6: Run `smt_vectors` green** and crate unit tests; compare literal hashes exactly.

- [ ] **Step 7: Commit hash/node foundation.**

---

### Task 5: Canonical writes, storage-neutral source and deterministic batch transitions

**Files:**
- Create: `crates/oregon-contract-state/src/source.rs`
- Create: `crates/oregon-contract-state/src/write_set.rs`
- Create: `crates/oregon-contract-state/src/transition.rs`
- Create: `crates/oregon-contract-state/tests/security.rs`
- Modify: `crates/oregon-contract-state/src/lib.rs`

**Exact public interfaces:**

```rust
pub trait StateSource {
    fn get_node(&self, node_hash: &Hash256) -> Result<Option<StateNode>, StateError>;
    fn get_value(&self, value_hash: &Hash256) -> Result<Option<Vec<u8>>, StateError>;
}

pub struct StateWrite { /* key/value private */ }
pub struct StateWriteSet { /* domain + path-sorted writes private */ }

impl StateWrite {
    pub fn put(key: Vec<u8>, value: Vec<u8>) -> Self;
    pub fn delete(key: Vec<u8>) -> Self;
}

impl StateWriteSet {
    pub fn new(domain: CommitmentDomainId, writes: Vec<StateWrite>) -> Result<Self, StateError>;
    pub fn domain(&self) -> CommitmentDomainId;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

#[derive(Clone, Copy)]
pub struct DomainSnapshot {
    pub domain: CommitmentDomainId,
    pub root: Hash256,
}

pub struct StateTransition {
    pub domain: CommitmentDomainId,
    pub old_root: Hash256,
    pub new_root: Hash256,
    pub nodes: std::collections::BTreeMap<Hash256, StateNode>,
    pub values: std::collections::BTreeMap<Hash256, Vec<u8>>,
}

pub fn read_value<S: StateSource>(
    source: &S,
    snapshot: DomainSnapshot,
    key: &[u8],
) -> Result<Option<Vec<u8>>, StateError>;

pub fn apply_write_set<S: StateSource>(
    source: &S,
    snapshot: DomainSnapshot,
    writes: &StateWriteSet,
) -> Result<StateTransition, StateError>;
```

- [ ] **Step 1: Implement `StateWriteSet::new`.** Validate before allocation growth, derive path keys, sort ascending path bytes, reject duplicate path keys and >65,536 writes. Keep raw canonical key/value with derived path key internally.

- [ ] **Step 2: Add tests for input-order independence and duplicates.** Same unique writes in reversed/random order must produce identical normalized order/root; same path twice must return `DuplicatePath`.

- [ ] **Step 3: Implement validated node loading.** For a non-default hash, `StateSource` must return a node; recompute its domain hash and compare requested hash; branch depth must equal traversal depth. Missing/hash/depth/type mismatch returns typed corruption error.

- [ ] **Step 4: Implement `read_value`.** Traverse exactly 256 MSB-first levels. If an internal root equals the deterministic empty hash for that depth, return `None` without source access. At a populated leaf verify path key, retrieve value blob and re-hash it before returning bytes.

- [ ] **Step 5: Implement deterministic recursive batch update rather than N independent full-path updates.** Internal helper shape:

```rust
fn apply_subtree<S: StateSource>(
    source: &S,
    domain: CommitmentDomainId,
    depth: usize,
    current: Hash256,
    writes: &[NormalizedWrite],
    output: &mut TransitionBuilder,
) -> Result<Hash256, StateError>
```

At each depth, split the already path-sorted slice into contiguous `0` and `1` groups, recurse only into touched groups, reuse untouched child hashes, collapse both-default children to `empty[depth]`, and insert only final newly reachable branch/leaf records into the transition BTreeMaps.

- [ ] **Step 6: Define leaf behavior.** At depth 256 there is exactly one normalized path. Delete of empty is no-op; put computes value hash + leaf; delete populated returns empty; same-value put keeps the same root. Existing non-empty leaf record must hash correctly and match the requested path.

- [ ] **Step 7: Add corruption/security tests.** Missing branch, wrong node hash, wrong depth, leaf at internal depth, branch at leaf depth, missing value and value-hash mismatch must all fail closed.

- [ ] **Step 8: Add root transition vectors.** Pin one leaf, two diverging paths, long shared prefix, update, delete-back-to-empty, delete absent, and order independence against literal JSON roots.

- [ ] **Step 9: Commit transition engine.** No storage schema or chainstate edits.

---

### Task 6: Canonical compressed proofs

**Files:**
- Create: `crates/oregon-contract-state/src/proof.rs`
- Create: `crates/oregon-contract-state/tests/proofs.rs`
- Modify: `crates/oregon-contract-state/src/lib.rs`

**Exact API:**

```rust
pub const SMT_PROOF_VERSION: u16 = 1;
pub const SMT_PROOF_BITMAP_BYTES: usize = 32;
pub const MAX_SMT_PROOF_BYTES: usize = 8_226;

pub struct SparseMerkleProofV1 {
    sibling_bitmap: [u8; 32],
    siblings: Vec<Hash256>,
}

impl SparseMerkleProofV1 {
    pub fn encode(&self) -> Vec<u8>;
    pub fn decode(domain: CommitmentDomainId, bytes: &[u8]) -> Result<Self, StateError>;
}

pub fn prove<S: StateSource>(
    source: &S,
    snapshot: DomainSnapshot,
    key: &[u8],
) -> Result<(Option<Vec<u8>>, SparseMerkleProofV1), StateError>;

pub fn verify_proof(
    domain: CommitmentDomainId,
    key: &[u8],
    value: Option<&[u8]>,
    proof: &SparseMerkleProofV1,
    expected_root: Hash256,
) -> Result<(), StateError>;
```

- [ ] **Step 1: Write failing literal proof tests before implementation.** Pin concrete membership and non-membership proof bytes from JSON, including a proof with no explicit siblings and one with multiple explicit siblings.

- [ ] **Step 2: Implement wire decode.** Require `len >= 34`, `len <= 8226`, version exactly 1, `(len - 34) % 32 == 0`, and explicit sibling count exactly equals bitmap popcount. Bitmap depth bits are MSB-first and siblings are stored in increasing depth order.

- [ ] **Step 3: Enforce canonical domain-specific compression during decode.** Walk set bits in increasing depth order and reject an explicit sibling equal to that domain's `empty[depth + 1]` as `RedundantDefaultSibling`.

- [ ] **Step 4: Implement verification.** Derive membership leaf or empty leaf, reconstruct from depth 255 down to 0, select left/right by MSB path bit, use explicit sibling for set bit or domain default for clear bit, and require exact expected root. Return `InvalidProof` on mismatch.

- [ ] **Step 5: Implement proof construction.** Traverse from root; collect each depth's sibling; omit deterministic default siblings; when an internal subtree is empty, finish remaining depths with omitted defaults without source reads. For membership verify leaf and value blob; for non-membership end at deterministic empty.

- [ ] **Step 6: Add wrong-context tests.** Wrong domain, key, value, root, sibling, bitmap, truncation, trailing bytes and redundant default sibling all fail.

- [ ] **Step 7: Run proof + SMT + security tests green** and commit.

---

### Task 7: Mutation gates and adversarial hardening

**Files:**
- Create: `scripts/verify_contract_state_mutations.py`
- Modify: `.github/workflows/oregon-rust.yml`
- Extend: `crates/oregon-primitives/tests/state_commitments.rs`
- Extend: `crates/oregon-contract-state/tests/security.rs`
- Extend: `crates/oregon-contract-state/tests/proofs.rs`

**Mutation runner contract:**

Use the same safety pattern as the existing execution-address/envelope mutation runners:

- require clean checkout;
- mutate one exact source snippet at a time;
- run a named targeted test command;
- kill only when that intended test returns Rust test failure code `101`;
- compilation errors do not count as kills;
- restore changed source in `finally`;
- verify clean source after each mutant and after the run.

- [ ] **Step 1: Add mutant 1 — omit `domain_id` from path-key hash.** Target cross-domain separation test.

- [ ] **Step 2: Add mutant 2 — use LSB-first path bit.** Target frozen bit-order/root vector test.

- [ ] **Step 3: Add mutant 3 — omit depth from branch hash.** Target depth-commitment/golden-root test.

- [ ] **Step 4: Add mutant 4 — map `Some([])` to absence.** Target present-empty-value test.

- [ ] **Step 5: Add mutant 5 — allow duplicate path key.** Target canonical write-set test.

- [ ] **Step 6: Add mutant 6 — missing non-empty node becomes empty.** Target corruption fail-closed test.

- [ ] **Step 7: Add mutant 7 — accept redundant default proof sibling.** Target canonical proof test.

- [ ] **Step 8: Add mutant 8 — omit scheme id from aggregate identity/encoding.** Target scheme-binding aggregate test.

- [ ] **Step 9: Add mutant 9 — accept unsorted/duplicate descriptor set.** Target aggregate canonicality test.

- [ ] **Step 10: Add mutant 10 — proof verification ignores domain.** Target wrong-domain proof test.

- [ ] **Step 11: Add hostile-input/proptest cases.** Random proof bytes up to 8,500 bytes and random small keys/values must never panic; oversized buffers fail before expensive traversal/copy.

- [ ] **Step 12: Wire CI order.** Focused primitive commitment and contract-state tests run before full workspace; mutation gate runs after full workspace green.

Command for the mutation step:

```bash
python3 scripts/verify_contract_state_mutations.py
```

- [ ] **Step 13: Require `10/10` mutation kills** before checkpointing.

---

### Task 8: Full verification, inherited gates and persistent checkpoint

**Files:**
- Create: `docs/checkpoints/OREGON_CONTRACT_STATE_PROGRESS.md`
- Modify: `HANDOFF.md`
- Complete checkboxes in this plan only after evidence exists.

- [ ] **Step 1: Run focused exact-head contracts.**

```bash
cargo +1.85.0 test --locked -p oregon-primitives --test state_commitments
cargo +1.85.0 test --locked -p oregon-contract-state --test smt_vectors
cargo +1.85.0 test --locked -p oregon-contract-state --test proofs
cargo +1.85.0 test --locked -p oregon-contract-state --test security
```

- [ ] **Step 2: Run full inherited Oregon Rust gates on the exact implementation head.** Required success: architecture scan, execution address/envelope contracts, new state contracts, full workspace `--all-targets`, address mutations 3/3, envelope mutations 9/9, state mutations 10/10, chainstate rustdoc, workspace docs, rustfmt and Clippy `-D warnings`.

- [ ] **Step 3: Run inherited RandomX architecture/full-light parity on the exact code state where `oregon-primitives` changed.** Require x86 and ARM success. If the stacked PR topology does not naturally trigger them, use the existing temporary branch-trigger technique, record the successful checkpoint SHA/run ids, then remove temporary trigger entries and re-run final Oregon Rust CI on the clean descendant head.

- [ ] **Step 4: Verify non-activation diff.** PR/file diff must not include current `block.rs`, `transaction.rs`, `oregon-storage` schema/db/batch changes, `oregon-chainstate` behavior changes, mempool, node, RPC or VM execution.

- [ ] **Step 5: Write checkpoint evidence.** Record:
  - accepted main `bf7675bfe17182f77d4c43e2bcbd0c283709d799`;
  - Stage 1 clean head `b97f9d3af9e2c9c4011750cfb69cce8fd9117a8a`;
  - Stage 2 design/spec commit;
  - expected-red SHA/run/job for primitives and contract-state shell;
  - verified implementation SHA/tree;
  - exact Rust CI run/job;
  - `10/10` state mutation result;
  - inherited RandomX run ids;
  - non-activation limitations.

- [ ] **Step 6: Update `HANDOFF.md`.** Mark Stage 2 complete only after the final clean descendant head is green. Exact next action becomes Execution Architecture §27 Stage 3: normalized resource weight, fee escrow/state transition and UTXO reserve conservation. Do not repeat Stage 1 or Stage 2 work.

- [ ] **Step 7: Verify remote identity.** Fetch branch ref, final commit/tree, checkpoint file, handoff, PR state and exact workflow results from GitHub before reporting saved/verified.

- [ ] **Step 8: Keep integration separate.** Stage 2 PR remains stacked/draft as appropriate; do not merge `main` as part of this task.

## Self-review

- Spec coverage: descriptor ids/bytes, aggregate root, fixed-depth SMT, MSB path order, empty ladder, immutable nodes, write normalization, storage-neutral source, batch transition, proof codec, corruption behavior, resource ceilings, EVM separation, reorg model, vectors and mutation gates each map to a concrete task.
- Scope: one subsystem (`oregon-contract-state` + its primitive descriptor boundary); persistence/fees/runtime/EVM remain later stages.
- Type consistency: `CommitmentDomainId`, `CommitmentSchemeId`, `DomainSnapshot`, `StateNode`, `StateSource`, `StateWriteSet`, `StateTransition`, `SparseMerkleProofV1` and function signatures are defined once and reused consistently.
- Placeholder scan: implementation steps contain no unresolved implementation TODO/TBD. Literal vector creation explicitly requires concrete independent values before test publication.
- Activation check: no current block/header/transaction/storage/chainstate path is modified by this plan.
