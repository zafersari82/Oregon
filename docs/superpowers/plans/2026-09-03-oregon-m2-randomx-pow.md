# Oregon M2 RandomX PoW Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Oregon v1 RandomX v2.0.1 proof-of-work verification with a pinned upstream engine, Oregon-specific Argon salt, deterministic key schedule, exact PoW input bytes, cross-architecture vectors, and mutation-sensitive acceptance evidence.

**Architecture:** Keep consensus orchestration in Rust and isolate upstream RandomX C/C++ behind a small `oregon-pow` crate. `oregon-pow` depends only on `oregon-primitives` and never imports `oregon-consensus`; target comparison remains in `oregon-consensus`, avoiding a dependency cycle. Pin upstream RandomX as a git submodule at commit `aaafe71322df6602c21a5c72937ac284724ae561`; build from a copied `OUT_DIR` tree after verifying and replacing exactly the upstream Argon salt line.

**Tech Stack:** Rust 1.85.0 / edition 2024, BLAKE3, `cmake` build dependency, RandomX v2.0.1 C API, GitHub Actions Linux x64 + Linux ARM64.

**Spec:** `docs/superpowers/specs/2026-09-02-oregon-pow-consensus-v1-design.md`

## Global Constraints

- Upstream RandomX is exactly tag `v2.0.1`, commit `aaafe71322df6602c21a5c72937ac284724ae561`.
- Oregon RandomX Argon salt is exactly `OREGON-RANDOMX-V1`.
- No consensus-affecting RandomX parameter other than the salt may change.
- Key epoch is 864 blocks; activation delay is 24 blocks.
- Key block height is `0` for `h < 888`, otherwise `((h - 24) / 864) * 864`.
- RandomX key is `BLAKE3("OREGON/RANDOMX-KEY/V1\0" || key_block_id_bytes)`.
- PoW input is `b"OREGON/POW/V1\0" || canonical_114_byte_block_header`.
- RandomX output and target are unsigned 256-bit little-endian integers; PoW is valid iff `pow_value <= target`.
- `oregon-pow` MUST NOT depend on `oregon-consensus`.
- Validators use light/cache mode by default. JIT is optional and must use secure/W^X when enabled.
- Main test suites must not allocate the 2+ GiB full dataset; full-memory parity has a dedicated acceptance workflow.
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

```rust
pub const RANDOMX_UPSTREAM_COMMIT: &str = "aaafe71322df6602c21a5c72937ac284724ae561";
pub const OREGON_RANDOMX_ARGON_SALT: &str = "OREGON-RANDOMX-V1";
```

- [ ] **Step 1: Create crate skeleton and RED provenance test**

Create the crate manifest/workspace membership and this test in `src/lib.rs`, but do not define the constants yet:

```rust
#[cfg(test)]
mod tests {
    use super::{OREGON_RANDOMX_ARGON_SALT, RANDOMX_UPSTREAM_COMMIT};

    #[test]
    fn randomx_provenance_is_frozen() {
        assert_eq!(RANDOMX_UPSTREAM_COMMIT, "aaafe71322df6602c21a5c72937ac284724ae561");
        assert_eq!(OREGON_RANDOMX_ARGON_SALT, "OREGON-RANDOMX-V1");
    }
}
```

Run the workspace tests and require compile failure for the two missing constants.

- [ ] **Step 2: Add exact upstream gitlink and submodule metadata**

`.gitmodules`:

```ini
[submodule "vendor/RandomX"]
	path = vendor/RandomX
	url = https://github.com/tevador/RandomX.git
```

`vendor/RandomX` gitlink SHA MUST be exactly `aaafe71322df6602c21a5c72937ac284724ae561`.

- [ ] **Step 3: Implement deterministic build copy and exact salt patch**

`build.rs` must recursively copy `vendor/RandomX` to `OUT_DIR/randomx-oregon`, read copied `src/configuration.h`, require exactly one upstream line

```c
#define RANDOMX_ARGON_SALT         "RandomX\x03"
```

and replace it with

```c
#define RANDOMX_ARGON_SALT         "OREGON-RANDOMX-V1"
```

Then use `cmake::Config::new(copied_root).profile("Release").build()` to build/install the pinned copied tree. The checked-out submodule MUST remain byte-for-byte untouched. Emit search paths for `${dst}/lib` and `${dst}/lib64` and the platform C++ runtime (`stdc++` on Linux/GNU, `c++` on Apple; MSVC needs no explicit C++ runtime link).

- [ ] **Step 4: Define provenance constants and initialize submodules in CI**

CI checkout uses:

```yaml
- uses: actions/checkout@v4
  with:
    submodules: recursive
```

Push branches include `oregon-v1-m2-randomx-pow`.

- [ ] **Step 5: GREEN gate and reviewer check**

Require workspace test/fmt/clippy success. Verify gitlink SHA, exact source URL, checked-out upstream salt unchanged, copied-build salt changed exactly once, and no other RandomX configuration edit.

---

### Task 2: Safe Light-Mode RandomX Rust Wrapper

**Files:** `crates/oregon-pow/src/ffi.rs`, `engine.rs`, `lib.rs`.

**Produces:**

```rust
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PowError {
    #[error("RandomX cache allocation failed")]
    CacheAllocationFailed,
    #[error("RandomX VM allocation failed")]
    VmAllocationFailed,
}

pub struct LightEngine { /* private owners */ }
impl LightEngine {
    pub fn new(key: [u8; 32]) -> Result<Self, PowError>;
    pub fn hash(&mut self, input: &[u8]) -> [u8; 32];
}
```

- [ ] **Step 1: RED determinism/sensitivity tests** — same key/input repeats exactly; changing key changes hash; changing input changes hash.
- [ ] **Step 2: Bind only required C API** — cache alloc/init/release, VM create/destroy, calculate hash. Baseline flags are exactly `RANDOMX_FLAG_V2` (128), with no JIT/full-memory flag.
- [ ] **Step 3: RAII** — VM destroyed before cache; raw pointers and `unsafe` remain private.
- [ ] **Step 4: Gate/review** — null allocation handling, drop order, V2 flag cannot be omitted through public API.

---

### Task 3: Oregon RandomX Key Schedule and Domain Separation

**Files:** `crates/oregon-pow/src/key.rs`, `Cargo.toml`, `lib.rs`.

**Produces:**

```rust
pub const RANDOMX_KEY_EPOCH: u64 = 864;
pub const RANDOMX_KEY_DELAY: u64 = 24;
pub fn key_block_height(candidate_height: u64) -> u64;
pub fn derive_randomx_key(key_block_id: Hash256) -> [u8; 32];
```

- [ ] **Step 1: RED boundary tests** — `0→0`, `1→0`, `887→0`, `888→864`, `1751→864`, `1752→1728`.
- [ ] **Step 2: RED key-domain tests** — deterministic; one changed block-ID byte changes key.
- [ ] **Step 3: Implement exact formula/domain** — bytes are `OREGON/RANDOMX-KEY/V1\0 || Hash256 internal bytes`.
- [ ] **Step 4: Gate/review** — no miner-provided arbitrary key API.

---

### Task 4: Exact Oregon PoW Input and Raw Hash Value

**Files:** `crates/oregon-pow/src/verify.rs`, `Cargo.toml`, `lib.rs`.

**Produces:**

```rust
pub const POW_DOMAIN: &[u8] = b"OREGON/POW/V1\0";
pub fn pow_input(header: &BlockHeader) -> Vec<u8>;
pub fn pow_value_le(hash: [u8; 32]) -> BigUint;
pub fn hash_header(engine: &mut LightEngine, header: &BlockHeader) -> [u8; 32];
```

- [ ] **Step 1: RED layout tests** — input length is `POW_DOMAIN.len()+114`; suffix equals `header.encode()` exactly; changing nonce changes input.
- [ ] **Step 2: RED endian test** — crafted `[1,0,..]` decodes to integer 1 while `[0,1,0,..]` decodes to 256.
- [ ] **Step 3: Implement without consensus dependency** — use `BigUint::from_bytes_le`; never import `Target` or `oregon-consensus`.
- [ ] **Step 4: Gate/review** — exact NUL-terminated domain and canonical header only.

---

### Task 5: Connect Consensus Target to RandomX Verification

**Files:** `crates/oregon-consensus/Cargo.toml`, `src/pow.rs`, `src/lib.rs`.

**Produces:**

```rust
pub fn hash_meets_target(hash: [u8; 32], target: Target) -> bool;

pub fn validate_header_pow(
    header: &BlockHeader,
    facts: &PrePowHeaderFacts,
    key_block_id: Hash256,
) -> Result<[u8; 32], ConsensusError>;
```

- [ ] **Step 1: RED endian/threshold tests** — equality passes; target+1 fails; crafted bytes prove numeric little-endian comparison rather than lexicographic comparison.
- [ ] **Step 2: Add typed `InvalidProofOfWork` error**.
- [ ] **Step 3: Implement orchestration** — derive key from `key_block_id`, create light engine, hash exact header, compare `oregon_pow::pow_value_le(hash)` to `facts.target.to_biguint()`.
- [ ] **Step 4: Gate/review** — dependency direction is `consensus -> pow -> primitives`, never cyclic; cheap pre-PoW validation stays separate.

---

### Task 6: Golden RandomX Vectors and x64/ARM64 Parity

**Files:** `tests/vectors/randomx-m2-v1.json`, `crates/oregon-pow/tests/golden_vectors.rs`, `.github/workflows/oregon-randomx-parity.yml`.

- [ ] **Step 1: Golden consumer RED before fixture** — fail specifically because fixture is absent.
- [ ] **Step 2: Freeze exact vector** — fixture records upstream commit, Oregon salt, key-block ID, derived key, canonical header hex, PoW input hex and expected 32-byte RandomX hash.
- [ ] **Step 3: Cross-architecture CI** — same checked vector test on `ubuntu-24.04` x64 and `ubuntu-24.04-arm` ARM64.
- [ ] **Step 4: Dedicated full-memory parity** — workflow-dispatch acceptance job initializes one full dataset for the frozen key and proves full-memory hash equals checked light hash. This job is not part of ordinary workspace CI.

---

### Task 7: Mutation Sensitivity and M2 Acceptance

**Files:** `docs/checkpoints/OREGON_V1_M2_RANDOMX_POW.md`, `docs/checkpoints/OREGON_V0_PROTOCOL_PROGRESS.md`.

- [ ] **Step 1: Mutation A** — corrupt 887/888 key boundary; key/vector tests must fail.
- [ ] **Step 2: Mutation B** — interpret PoW hash big-endian in consensus; endian/threshold tests must fail.
- [ ] **Step 3: Mutation C** — change one byte of PoW domain or Oregon salt; golden vector must fail.
- [ ] **Step 4: Exact-head acceptance** — workspace test/fmt/clippy + x64/ARM64 vector parity + full-memory parity all successful on recorded SHAs/runs.
- [ ] **Step 5: Recovery branch** — create `oregon-v1-checkpoint-m2-randomx-pow-accepted-2026-09-03` only from exact successful final head.

## Plan Self-Review Record

**Spec coverage:** upstream pin/salt -> Task 1; safe light engine -> Task 2; key schedule -> Task 3; exact PoW input/raw value -> Task 4; target validation -> Task 5; cross-mode/architecture vectors -> Task 6; mutations/acceptance -> Task 7.

**Dependency review:** `oregon-pow -> oregon-primitives`; `oregon-consensus -> oregon-pow`; `oregon-pow` never imports `Target` or `oregon-consensus`, so no crate cycle exists.

**Scope exclusions:** M3 UTXO/signatures/maturity, M4 persistence/reorg, M5 genesis/address profile, M6 P2P, M7 mempool/mining/RPC.

**Acceptance principle:** M2 is not accepted because RandomX merely compiles. Pinned-source provenance, Oregon salt, key schedule, LE comparison, cross-architecture vectors, light/full parity and deliberate mutation detection are all required.