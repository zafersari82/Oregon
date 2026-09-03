# Oregon M2 RandomX PoW Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Oregon v1 RandomX v2.0.1 proof-of-work verification with a pinned upstream engine, Oregon-specific Argon salt, deterministic key schedule, exact PoW input bytes, cross-architecture vectors, and mutation-sensitive acceptance evidence.

**Architecture:** Keep consensus orchestration in Rust and isolate upstream RandomX C/C++ behind a small `oregon-pow` crate. Pin upstream RandomX as a git submodule at commit `aaafe71322df6602c21a5c72937ac284724ae561`; the build copies that source into `OUT_DIR`, verifies the expected upstream salt, replaces only `"RandomX\\x03"` with `"OREGON-RANDOMX-V1"`, then builds the copied source with CMake. Consensus-facing APIs never expose raw C pointers.

**Tech Stack:** Rust 1.85.0 / edition 2024, BLAKE3, `cmake` build dependency, RandomX v2.0.1 C API, GitHub Actions Linux x64 + Linux ARM64.

**Spec:** `docs/superpowers/specs/2026-09-02-oregon-pow-consensus-v1-design.md`

## Global Constraints

- Upstream RandomX is exactly tag `v2.0.1`, commit `aaafe71322df6602c21a5c72937ac284724ae561`.
- Oregon RandomX Argon salt is exactly `OREGON-RANDOMX-V1`.
- No consensus-affecting RandomX parameter other than the salt may change.
- Key epoch is 864 blocks; activation delay is 24 blocks.
- Key block height is `0` for `h < 888`, otherwise `((h - 24) / 864) * 864`.
- RandomX key is `BLAKE3("OREGON/RANDOMX-KEY/V1\\0" || key_block_id_bytes)`.
- PoW input is `b"OREGON/POW/V1\\0" || canonical_114_byte_block_header`.
- RandomX output and target are unsigned 256-bit little-endian integers; PoW is valid iff `pow_value <= target`.
- Validators use light/cache mode by default. JIT is optional and must use secure/W^X when enabled.
- Main consensus tests must not require a 2+ GiB full dataset. Full-memory parity runs in a dedicated acceptance workflow.
- No hardware attestation, miner identity, enrollment, IP identity, or privileged mining path.

---

### Task 1: Pin and Build the Upstream RandomX Engine

**Files:**
- Create: `.gitmodules`
- Add gitlink: `vendor/RandomX`
- Create: `crates/oregon-pow/Cargo.toml`
- Create: `crates/oregon-pow/build.rs`
- Create: `crates/oregon-pow/src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `.github/workflows/oregon-rust.yml`

**Interfaces:**
- Produces `oregon-pow` crate with build-time link to `randomx`.
- Produces constants `RANDOMX_UPSTREAM_COMMIT` and `OREGON_RANDOMX_ARGON_SALT`.

- [ ] **Step 1: Add RED provenance test before the crate exists**

Add a workspace test that imports:

```rust
use oregon_pow::{OREGON_RANDOMX_ARGON_SALT, RANDOMX_UPSTREAM_COMMIT};

#[test]
fn randomx_provenance_is_frozen() {
    assert_eq!(RANDOMX_UPSTREAM_COMMIT, "aaafe71322df6602c21a5c72937ac284724ae561");
    assert_eq!(OREGON_RANDOMX_ARGON_SALT, "OREGON-RANDOMX-V1");
}
```

Run `cargo +1.85.0 test --locked --workspace --all-targets` and require failure because `oregon_pow` does not exist.

- [ ] **Step 2: Add exact upstream gitlink and submodule metadata**

`.gitmodules` must contain:

```ini
[submodule "vendor/RandomX"]
	path = vendor/RandomX
	url = https://github.com/tevador/RandomX.git
```

The gitlink SHA for `vendor/RandomX` must be exactly `aaafe71322df6602c21a5c72937ac284724ae561`.

- [ ] **Step 3: Implement deterministic build copy + one-line salt patch**

`build.rs` must:

1. Resolve `vendor/RandomX` from `CARGO_MANIFEST_DIR/../..`.
2. Recursively copy the submodule into `OUT_DIR/randomx-oregon`.
3. Read copied `src/configuration.h`.
4. Require exactly one occurrence of `#define RANDOMX_ARGON_SALT         "RandomX\\x03"`; otherwise panic with a provenance/config mismatch.
5. Replace exactly that line with `#define RANDOMX_ARGON_SALT         "OREGON-RANDOMX-V1"`.
6. Use `cmake::Config` to build the copied tree in Release mode with tests/benchmarks disabled by a minimal Oregon overlay if upstream CMake requires it; never edit the checked-out submodule.
7. Emit the library search path and platform C++ runtime link directives.

- [ ] **Step 4: Make CI initialize submodules**

Use `actions/checkout@v4` with:

```yaml
with:
  submodules: recursive
```

Add branch `oregon-v1-m2-randomx-pow` to the push gate when execution branch exists.

- [ ] **Step 5: GREEN gate and reviewer check**

Run full tests, rustfmt, clippy. Reviewer must verify the gitlink SHA, that upstream source is untouched, and that only the salt is patched in the copied build tree.

---

### Task 2: Safe Light-Mode RandomX Rust Wrapper

**Files:**
- Create: `crates/oregon-pow/src/ffi.rs`
- Create: `crates/oregon-pow/src/engine.rs`
- Modify: `crates/oregon-pow/src/lib.rs`

**Interfaces:**

```rust
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PowError {
    #[error("RandomX cache allocation failed")]
    CacheAllocationFailed,
    #[error("RandomX VM allocation failed")]
    VmAllocationFailed,
}

pub struct LightEngine { /* private FFI owners */ }
impl LightEngine {
    pub fn new(key: [u8; 32]) -> Result<Self, PowError>;
    pub fn hash(&mut self, input: &[u8]) -> [u8; 32];
}
```

- [ ] **Step 1: RED lifecycle/hash determinism tests**

Test that the same key/input hashed twice gives identical 32 bytes; changing one key byte changes output; changing one input byte changes output.

- [ ] **Step 2: Implement minimal FFI declarations**

Bind only `randomx_alloc_cache`, `randomx_init_cache`, `randomx_release_cache`, `randomx_create_vm`, `randomx_destroy_vm`, `randomx_calculate_hash`. Use `RANDOMX_FLAG_V2` plus portable/default flags for baseline verification. Keep all `unsafe` inside `ffi.rs`/`engine.rs`.

- [ ] **Step 3: RAII ownership**

`Drop` must destroy VM before releasing cache. Public API contains no raw pointer and no `unsafe` function.

- [ ] **Step 4: Full gate and review**

Run workspace test/fmt/clippy. Reviewer checks null-return handling, ownership order, and that no consensus code can choose RandomX v1 flags.

---

### Task 3: Oregon RandomX Key Schedule and Domain Separation

**Files:**
- Create: `crates/oregon-pow/src/key.rs`
- Modify: `crates/oregon-pow/Cargo.toml`
- Modify: `crates/oregon-pow/src/lib.rs`

**Interfaces:**

```rust
pub const RANDOMX_KEY_EPOCH: u64 = 864;
pub const RANDOMX_KEY_DELAY: u64 = 24;
pub fn key_block_height(candidate_height: u64) -> u64;
pub fn derive_randomx_key(key_block_id: Hash256) -> [u8; 32];
```

- [ ] **Step 1: RED boundary tests**

Freeze these values:

```text
h=0 -> 0
h=1 -> 0
h=887 -> 0
h=888 -> 864
h=1751 -> 864
h=1752 -> 1728
```

Also test that two different block IDs derive different keys and that repeated derivation is deterministic.

- [ ] **Step 2: Implement exact formula and BLAKE3 domain**

Hash bytes exactly as `OREGON/RANDOMX-KEY/V1\0 || Hash256 internal bytes`.

- [ ] **Step 3: Gate/review**

Reviewer checks off-by-one boundaries at 887/888 and 1751/1752 and confirms miners cannot provide arbitrary key bytes to consensus verification.

---

### Task 4: Exact Oregon PoW Input and Target Comparison

**Files:**
- Create: `crates/oregon-pow/src/verify.rs`
- Modify: `crates/oregon-pow/Cargo.toml`
- Modify: `crates/oregon-pow/src/lib.rs`

**Interfaces:**

```rust
pub const POW_DOMAIN: &[u8] = b"OREGON/POW/V1\0";
pub fn pow_input(header: &BlockHeader) -> Vec<u8>;
pub fn pow_value(hash: [u8; 32]) -> BigUint;
pub fn hash_meets_target(hash: [u8; 32], target: Target) -> bool;
pub fn verify_header_pow(
    engine: &mut LightEngine,
    header: &BlockHeader,
    target: Target,
) -> Result<[u8; 32], PowError>;
```

- [ ] **Step 1: RED byte-layout tests**

Assert PoW input length equals `POW_DOMAIN.len() + 114`; assert suffix equals `header.encode()` exactly. Test little-endian target comparison with crafted hashes where lexicographic byte order would give the wrong result.

- [ ] **Step 2: Implement integer comparison**

Use `BigUint::from_bytes_le` for RandomX output; compare numerically to `Target::to_biguint()`.

- [ ] **Step 3: Implement engine-backed verification**

Hash only the domain + canonical header. Do not hash transaction body or noncanonical reconstructed fields.

- [ ] **Step 4: Gate/review**

Reviewer checks domain bytes including NUL, exact 114-byte header use, `<=` not `<`, and no big-endian/lexicographic comparison.

---

### Task 5: Connect Pre-PoW Consensus Context to RandomX Verification

**Files:**
- Modify: `crates/oregon-consensus/Cargo.toml`
- Create: `crates/oregon-consensus/src/pow.rs`
- Modify: `crates/oregon-consensus/src/lib.rs`

**Interfaces:**

```rust
pub fn validate_header_pow(
    header: &BlockHeader,
    facts: &PrePowHeaderFacts,
    key_block_id: Hash256,
) -> Result<[u8; 32], ConsensusError>;
```

- [ ] **Step 1: RED tests**

Test that target mismatch is already rejected by pre-PoW context, while a PoW hash above the validated target maps to a typed `InvalidProofOfWork`. Use a test seam/trait only if necessary to avoid mining nonce loops in unit tests.

- [ ] **Step 2: Implement consensus orchestration**

Derive the key from `key_block_id`, create/reuse a light engine, hash the exact header input, then compare to `facts.target`. Consensus callers must not pass an arbitrary RandomX key.

- [ ] **Step 3: Gate/review**

Reviewer verifies validation order remains cheap pre-PoW first, expensive RandomX second.

---

### Task 6: Golden RandomX Vectors and x64/ARM64 Parity

**Files:**
- Create: `tests/vectors/randomx-m2-v1.json`
- Create: `crates/oregon-pow/tests/golden_vectors.rs`
- Create: `.github/workflows/oregon-randomx-parity.yml`

- [ ] **Step 1: Golden consumer RED before fixture**

Consumer reads key block ID/input/header vectors and recomputes key + light RandomX hash using only public APIs. First run must fail because fixture is absent.

- [ ] **Step 2: Freeze vector bytes from the patched v2.0.1 engine**

Fixture must include upstream commit, Oregon salt, key block ID, derived key, canonical header hex, PoW input hex, and expected 32-byte RandomX hash.

- [ ] **Step 3: Cross-architecture CI**

Run the same golden test on `ubuntu-24.04` and `ubuntu-24.04-arm`. Both jobs must produce the same checked vector. The repository is public; standard GitHub-hosted ARM64 Linux runners are available under `ubuntu-24.04-arm`.

- [ ] **Step 4: Full-memory parity job**

In a separate manually callable/acceptance job, initialize the full dataset once for the frozen vector key and prove full-memory output equals light-mode output. Do not make every normal PR allocate the dataset.

---

### Task 7: Mutation Sensitivity and M2 Acceptance

**Files:**
- Create: `docs/checkpoints/OREGON_V1_M2_RANDOMX_POW.md`
- Modify: `docs/checkpoints/OREGON_V0_PROTOCOL_PROGRESS.md`

- [ ] **Step 1: Mutation A — key schedule off-by-one**

On throwaway branch change `h < 888` to `h < 887` or equivalent boundary corruption. Run key/vector tests and require failure.

- [ ] **Step 2: Mutation B — PoW byte order**

Change RandomX output interpretation from little-endian to big-endian. Crafted target-comparison and/or golden acceptance test must fail.

- [ ] **Step 3: Mutation C — domain/salt drift**

Change one byte in `OREGON/POW/V1\0` or `OREGON-RANDOMX-V1`; golden vector must fail.

- [ ] **Step 4: Exact-head acceptance**

Require clean execution branch exact SHA to pass workspace test/fmt/clippy, x64/ARM64 golden parity, and full-memory parity acceptance job. Record all exact SHAs/run IDs.

- [ ] **Step 5: Recovery checkpoint**

Create `oregon-v1-checkpoint-m2-randomx-pow-accepted-2026-09-03` only from the exact fully successful final head.

## Plan Self-Review Record

**Spec coverage:** RandomX pin/salt -> Task 1; safe engine -> Task 2; key schedule/domain -> Task 3; PoW input and LE comparison -> Task 4; consensus integration -> Task 5; cross-mode/cross-architecture vectors -> Task 6; mutation and checkpoint -> Task 7.

**Type consistency:** `Hash256` supplies key-block identity, `BlockHeader` supplies canonical 114-byte input, `Target` remains the only consensus target type, and raw RandomX hashes remain `[u8; 32]` until numeric comparison.

**Scope exclusions:** M2 does not implement UTXO/signatures/coinbase maturity (M3), persistence/reorg (M4), genesis/address profile (M5), P2P (M6), or mempool/mining RPC (M7). Full dataset is acceptance/mining infrastructure, not default validator state.

**Acceptance principle:** M2 is not accepted merely because RandomX compiles. The exact pinned/modified engine must be vector-stable across x64 and ARM64, light/full modes must agree, and mutations to key schedule, byte order, or domain/salt must be detected.