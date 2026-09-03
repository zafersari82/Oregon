# Oregon M1 Consensus Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first Oregon v1 consensus crate with exact target arithmetic, issuance, one-time founder/coinbase rules, ASERT, median-time-past, chain-work calculation, pre-PoW header-context validation, and non-genesis block structural validation.

**Architecture:** Add a focused `oregon-consensus` crate above frozen `oregon-primitives`. The existing 32-byte `BlockHeader::difficulty_commitment` remains the wire representation; `oregon-consensus` interprets it as an unsigned little-endian target and uses exact big-integer arithmetic for ASERT/work. M1 deliberately excludes RandomX, UTXO/signature/maturity validation, persistence, P2P, and genesis generation.

**Tech Stack:** Rust 1.85.0, edition 2024, `oregon-primitives`, `thiserror`, `num-bigint = 0.5.1`, `num-traits = 0.2.19`, test-only `proptest`, `serde`, `serde_json`.

**Spec:** `docs/superpowers/specs/2026-09-02-oregon-pow-consensus-v1-design.md`

## Global Constraints

- Execution branch: `oregon-v1-m1-consensus-core`, created from the approved spec/plan head.
- Rust edition 2024; MSRV 1.85.0.
- No floating-point value may influence consensus.
- No `unsafe` in Oregon consensus code.
- No Bitcoin source imports, `.cpp/.h` copying, bincode, postcard, or generic binary consensus codec.
- Do not change v0 canonical transaction/block bytes, hash domains, 114-byte header layout, or v0 golden vectors.
- Target bytes are exactly unsigned little-endian `[u8; 32]`; valid range is `1..=POW_LIMIT`.
- Height 1 target is exactly `INITIAL_TARGET`.
- For height `h >= 2`, ASERT uses fixed anchor target `INITIAL_TARGET`, genesis timestamp as anchor-parent time, parent timestamp as evaluation time, 300-second target interval, 21,600-second half-life, 65,536 radix, and the coefficients frozen in the v1 spec.
- Initial subsidy is `237,500,000` base units; halving interval `200,000`; exact scheduled mining issuance `94,999,997,000,000` base units.
- Height 1 founder output is exactly output 0, exactly `5,000,000,000,000` base units, locking program `0x01 || FOUNDER_KEY_COMMITMENT`.
- Coinbase maturity belongs to M3 because M1 has no UTXO state.
- Maximum canonical non-genesis block size is `1,048,576` bytes; each transaction in a non-genesis block, including coinbase, is at most `102,400` bytes.
- MTP window contains 1..=11 parent/ancestor timestamps and uses sorted index `len / 2`.
- M1 header validation is explicitly pre-PoW. RandomX verification is M2 and must not be implied by M1 APIs/docs.
- Commit `Cargo.lock` after dependency resolution.
- Every task: RED -> GREEN -> full workspace gate -> review -> commit.

---

## File Map

- Modify `Cargo.toml` — add `crates/oregon-consensus` workspace member.
- Modify `.github/workflows/oregon-rust.yml` — run on `oregon-v1-m1-consensus-core`.
- Create `crates/oregon-consensus/Cargo.toml`.
- Create `crates/oregon-consensus/src/error.rs` — typed M1 errors.
- Create `crates/oregon-consensus/src/params.rs` — fixed constants and `ConsensusParams`.
- Create `crates/oregon-consensus/src/target.rs` — target bytes/bounds.
- Create `crates/oregon-consensus/src/emission.rs` — subsidy schedule.
- Create `crates/oregon-consensus/src/coinbase.rs` — coinbase/founder/reward ceiling.
- Create `crates/oregon-consensus/src/asert.rs` — exact ASERT.
- Create `crates/oregon-consensus/src/time.rs` — MTP.
- Create `crates/oregon-consensus/src/work.rs` — block/cumulative work value.
- Create `crates/oregon-consensus/src/header.rs` — parent/time/target pre-PoW context.
- Create `crates/oregon-consensus/src/block.rs` — block size/Merkle/coinbase structure.
- Create `crates/oregon-consensus/src/lib.rs` — exports only.
- Create `crates/oregon-consensus/tests/golden_vectors.rs`.
- Create `tests/vectors/consensus-m1-v1.json`.
- Create `docs/checkpoints/OREGON_V1_M1_CONSENSUS_CORE.md` at acceptance.
- Append one pointer section to `docs/checkpoints/OREGON_V0_PROTOCOL_PROGRESS.md` after M1 acceptance.
- Generate and commit `Cargo.lock`.

---

### Task 1: Consensus Crate, Parameters, and Full-Width Target

**Files:** `Cargo.toml`, `.github/workflows/oregon-rust.yml`, `crates/oregon-consensus/Cargo.toml`, `src/lib.rs`, `src/error.rs`, `src/params.rs`, `src/target.rs`, `Cargo.lock`.

**Produces:**

```rust
pub struct Target([u8; 32]);
pub struct ConsensusParams {
    pub pow_limit: Target,
    pub initial_target: Target,
    pub founder_key_commitment: [u8; 32],
}
```

- [ ] **Step 1: Create execution branch**

```bash
git switch oregon-v1-consensus-design
git pull --ff-only
git switch -c oregon-v1-m1-consensus-core
```

- [ ] **Step 2: Add failing target tests**

`target.rs` tests:

```rust
#[cfg(test)]
mod tests {
    use num_bigint::BigUint;
    use super::*;

    #[test]
    fn zero_target_is_invalid() {
        assert_eq!(Target::from_le_bytes([0; 32]), Err(ConsensusError::ZeroTarget));
    }

    #[test]
    fn little_endian_target_round_trips() {
        let mut bytes = [0u8; 32];
        bytes[0] = 0x34;
        bytes[1] = 0x12;
        let target = Target::from_le_bytes(bytes).unwrap();
        assert_eq!(target.to_le_bytes(), bytes);
        assert_eq!(target.to_biguint(), BigUint::from(0x1234u32));
    }

    #[test]
    fn more_than_256_bits_is_rejected() {
        let value = BigUint::from(1u8) << 256usize;
        assert_eq!(Target::from_biguint(&value), Err(ConsensusError::TargetExceeds256Bits));
    }
}
```

`params.rs` test:

```rust
#[test]
fn initial_target_cannot_exceed_pow_limit() {
    let pow_limit = Target::from_biguint(&BigUint::from(100u32)).unwrap();
    let initial = Target::from_biguint(&BigUint::from(101u32)).unwrap();
    assert_eq!(
        ConsensusParams::new(pow_limit, initial, [7u8; 32]),
        Err(ConsensusError::InitialTargetAbovePowLimit)
    );
}
```

- [ ] **Step 3: Create crate manifest and run RED**

`crates/oregon-consensus/Cargo.toml`:

```toml
[package]
name = "oregon-consensus"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true

[dependencies]
num-bigint = "=0.5.1"
num-traits = "=0.2.19"
oregon-primitives = { path = "../oregon-primitives" }
thiserror = "2"

[dev-dependencies]
proptest = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Root workspace members:

```toml
members = ["crates/oregon-primitives", "crates/oregon-consensus"]
```

Run:

```bash
cargo +1.85.0 test -p oregon-consensus target::tests params::tests --no-fail-fast
```

Expected: compile/test failure because target/params implementation does not exist.

- [ ] **Step 4: Implement target, params, errors**

`error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConsensusError {
    #[error("target must be non-zero")] ZeroTarget,
    #[error("target exceeds 256 bits")] TargetExceeds256Bits,
    #[error("target exceeds POW_LIMIT")] TargetAbovePowLimit,
    #[error("INITIAL_TARGET exceeds POW_LIMIT")] InitialTargetAbovePowLimit,
    #[error("consensus arithmetic overflow")] ArithmeticOverflow,
    #[error("invalid non-genesis height")] InvalidHeight,
    #[error("unexpected difficulty target")] UnexpectedTarget,
    #[error("invalid median-time-past window")] InvalidMtpWindow,
    #[error("block timestamp is not greater than median-time-past")] TimestampNotAfterMtp,
    #[error("previous block id does not match parent")] PreviousBlockMismatch,
    #[error("coinbase structure is invalid")] InvalidCoinbase,
    #[error("height-1 founder output is invalid")] InvalidFounderOutput,
    #[error("coinbase claims more than subsidy plus fees")] CoinbaseOverClaim,
    #[error("block exceeds v1 canonical byte limit")] BlockTooLarge,
    #[error("transaction {0} exceeds v1 canonical byte limit")] TransactionTooLarge(usize),
    #[error("non-genesis block has no transactions")] EmptyNonGenesisBlock,
    #[error("transaction root does not match header")] MerkleRootMismatch,
    #[error("normal transaction uses null outpoint")] NullOutpointInNormalTransaction,
    #[error("multiple coinbase-form transactions")] MultipleCoinbase,
}
```

`target.rs`:

```rust
use num_bigint::BigUint;
use num_traits::Zero;
use crate::ConsensusError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Target([u8; 32]);

impl Target {
    pub fn from_le_bytes(bytes: [u8; 32]) -> Result<Self, ConsensusError> {
        if bytes.iter().all(|b| *b == 0) { return Err(ConsensusError::ZeroTarget); }
        Ok(Self(bytes))
    }

    pub const fn to_le_bytes(self) -> [u8; 32] { self.0 }

    pub fn to_biguint(&self) -> BigUint { BigUint::from_bytes_le(&self.0) }

    pub fn from_biguint(value: &BigUint) -> Result<Self, ConsensusError> {
        if value.is_zero() { return Err(ConsensusError::ZeroTarget); }
        let bytes = value.to_bytes_le();
        if bytes.len() > 32 { return Err(ConsensusError::TargetExceeds256Bits); }
        let mut fixed = [0u8; 32];
        fixed[..bytes.len()].copy_from_slice(&bytes);
        Ok(Self(fixed))
    }

    pub fn validate_against(self, pow_limit: Target) -> Result<(), ConsensusError> {
        if self.to_biguint() > pow_limit.to_biguint() {
            return Err(ConsensusError::TargetAbovePowLimit);
        }
        Ok(())
    }
}
```

Do **not** derive `Ord`/`PartialOrd` for `Target`; lexicographic order of little-endian bytes is not numeric target order.

`params.rs`:

```rust
use crate::{ConsensusError, Target};

pub const TARGET_BLOCK_SECONDS: u64 = 300;
pub const ASERT_HALF_LIFE_SECONDS: i128 = 21_600;
pub const ASERT_RADIX: i128 = 65_536;
pub const HALVING_INTERVAL: u64 = 200_000;
pub const INITIAL_SUBSIDY_BASE_UNITS: u64 = 237_500_000;
pub const MAX_BLOCK_BYTES: usize = 1_048_576;
pub const MAX_TRANSACTION_BYTES: usize = 102_400;
pub const KEY_COMMIT_V1: u8 = 0x01;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsensusParams {
    pub pow_limit: Target,
    pub initial_target: Target,
    pub founder_key_commitment: [u8; 32],
}

impl ConsensusParams {
    pub fn new(pow_limit: Target, initial_target: Target, founder_key_commitment: [u8; 32])
        -> Result<Self, ConsensusError>
    {
        if initial_target.to_biguint() > pow_limit.to_biguint() {
            return Err(ConsensusError::InitialTargetAbovePowLimit);
        }
        Ok(Self { pow_limit, initial_target, founder_key_commitment })
    }
}
```

`lib.rs` at this task exports only implemented modules:

```rust
pub mod error;
pub mod params;
pub mod target;

pub use error::ConsensusError;
pub use params::ConsensusParams;
pub use target::Target;
```

- [ ] **Step 5: Enable CI, generate lockfile, run full gate**

Workflow push branches:

```yaml
branches: [oregon-v0-protocol, oregon-v1-m1-consensus-core]
```

Run:

```bash
cargo +1.85.0 generate-lockfile
cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock .github/workflows/oregon-rust.yml crates/oregon-consensus
git commit -m "feat: establish Oregon consensus target model"
```

Reviewer gate: LE interpretation, zero/256-bit rejection, no `Target` bytewise ordering, `INITIAL_TARGET <= POW_LIMIT`, v0 unchanged.

---

### Task 2: Exact Emission Schedule

**Files:** `src/emission.rs`, `src/lib.rs`.

**Produces:**

```rust
pub fn block_subsidy(height: u64) -> Result<Amount, ConsensusError>;
pub const SCHEDULED_MINING_ISSUANCE_BASE_UNITS: u64 = 94_999_997_000_000;
pub const SCHEDULED_TOTAL_WITH_FOUNDER_BASE_UNITS: u64 = 99_999_997_000_000;
```

- [ ] **Step 1: Add RED boundary tests**

```rust
#[test]
fn exact_halving_boundaries() {
    assert_eq!(block_subsidy(1).unwrap().base_units(), 237_500_000);
    assert_eq!(block_subsidy(200_000).unwrap().base_units(), 237_500_000);
    assert_eq!(block_subsidy(200_001).unwrap().base_units(), 118_750_000);
}

#[test]
fn era_27_is_last_positive_era() {
    assert_eq!(block_subsidy(27 * HALVING_INTERVAL + 1).unwrap().base_units(), 1);
    assert_eq!(block_subsidy(28 * HALVING_INTERVAL + 1).unwrap().base_units(), 0);
}

#[test]
fn scheduled_issuance_is_exact() {
    let mut total = 0u128;
    for era in 0..28u64 {
        total += u128::from(block_subsidy(era * HALVING_INTERVAL + 1).unwrap().base_units())
            * u128::from(HALVING_INTERVAL);
    }
    assert_eq!(total, 94_999_997_000_000);
    assert_eq!(MAX_SUPPLY_BASE_UNITS - SCHEDULED_TOTAL_WITH_FOUNDER_BASE_UNITS, 3_000_000);
}
```

Run and confirm failure:

```bash
cargo +1.85.0 test -p oregon-consensus emission::tests --no-fail-fast
```

- [ ] **Step 2: Implement integer-only schedule**

```rust
use oregon_primitives::{Amount, FOUNDER_ALLOCATION_BASE_UNITS};
use crate::{ConsensusError, params::{HALVING_INTERVAL, INITIAL_SUBSIDY_BASE_UNITS}};

pub const SCHEDULED_MINING_ISSUANCE_BASE_UNITS: u64 = 94_999_997_000_000;
pub const SCHEDULED_TOTAL_WITH_FOUNDER_BASE_UNITS: u64 =
    SCHEDULED_MINING_ISSUANCE_BASE_UNITS + FOUNDER_ALLOCATION_BASE_UNITS;

pub fn block_subsidy(height: u64) -> Result<Amount, ConsensusError> {
    if height == 0 {
        return Amount::from_base_units(0).map_err(|_| ConsensusError::ArithmeticOverflow);
    }
    let era = (height - 1) / HALVING_INTERVAL;
    let subsidy = if era >= 64 { 0 } else { INITIAL_SUBSIDY_BASE_UNITS >> era as u32 };
    Amount::from_base_units(subsidy).map_err(|_| ConsensusError::ArithmeticOverflow)
}
```

Add module/export to `lib.rs` only after implementation exists.

- [ ] **Step 3: Full gate and commit**

```bash
cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
git add crates/oregon-consensus/src/emission.rs crates/oregon-consensus/src/lib.rs
git commit -m "feat: define Oregon mining emission schedule"
```

Reviewer gate: `(height - 1) / 200000`, no top-up mint, exact `.03 OREG` unreachable slack.

---

### Task 3: Coinbase, Founder Grant, Reward Ceiling

**Files:** `src/coinbase.rs`, `src/lib.rs`.

**Produces:**

```rust
pub fn is_coinbase_form(tx: &Transaction) -> bool;
pub fn validate_coinbase(
    tx: &Transaction,
    height: u64,
    total_fees: Amount,
    params: &ConsensusParams,
) -> Result<(), ConsensusError>;
```

- [ ] **Step 1: Add RED tests**

Create a helper that uses one null-outpoint input, `sequence = u32::MAX`, and canonical `write_varint(height)` witness item. Tests must prove:

```rust
#[test]
fn height_one_founder_output_is_exact() { /* exact 5e12 + 0x01 + commitment -> Ok */ }
#[test]
fn founder_value_or_index_mutation_is_rejected() { /* -> InvalidFounderOutput */ }
#[test]
fn height_two_has_no_special_founder_mint() { /* 5e12 extra -> CoinbaseOverClaim */ }
#[test]
fn canonical_height_witness_is_required() { /* altered bytes -> InvalidCoinbase */ }
#[test]
fn underclaim_is_valid_but_overclaim_is_invalid() { /* subsidy+fees-1 Ok, +1 invalid */ }
```

Run:

```bash
cargo +1.85.0 test -p oregon-consensus coinbase::tests --no-fail-fast
```

Expected: failure before implementation.

- [ ] **Step 2: Implement exact structural/reward rules**

Core logic:

```rust
pub fn is_coinbase_form(tx: &Transaction) -> bool {
    tx.inputs.len() == 1
        && tx.inputs[0].previous_txid == Hash256::from_bytes([0u8; 32])
        && tx.inputs[0].previous_output_index == u32::MAX
}

pub fn validate_coinbase(
    tx: &Transaction,
    height: u64,
    total_fees: Amount,
    params: &ConsensusParams,
) -> Result<(), ConsensusError> {
    if height == 0 || tx.version != 1 || tx.lock_time != 0 || !is_coinbase_form(tx) {
        return Err(ConsensusError::InvalidCoinbase);
    }
    let input = &tx.inputs[0];
    if input.sequence != u32::MAX || input.witness.is_empty() {
        return Err(ConsensusError::InvalidCoinbase);
    }
    let mut expected_height = Vec::new();
    write_varint(height, &mut expected_height);
    if input.witness[0] != expected_height { return Err(ConsensusError::InvalidCoinbase); }

    let miner_start = if height == 1 {
        let founder = tx.outputs.first().ok_or(ConsensusError::InvalidFounderOutput)?;
        let mut expected_program = vec![KEY_COMMIT_V1];
        expected_program.extend_from_slice(&params.founder_key_commitment);
        if founder.value.base_units() != FOUNDER_ALLOCATION_BASE_UNITS
            || founder.locking_program != expected_program
        {
            return Err(ConsensusError::InvalidFounderOutput);
        }
        1
    } else { 0 };

    let miner_claim = tx.outputs[miner_start..].iter().try_fold(0u64, |sum, output| {
        sum.checked_add(output.value.base_units()).ok_or(ConsensusError::ArithmeticOverflow)
    })?;
    let ceiling = block_subsidy(height)?.base_units()
        .checked_add(total_fees.base_units())
        .ok_or(ConsensusError::ArithmeticOverflow)?;
    if miner_claim > ceiling { return Err(ConsensusError::CoinbaseOverClaim); }
    Ok(())
}
```

Heights greater than 1 may have zero coinbase outputs, representing full under-claim. Height 1 cannot because founder output 0 is mandatory.

- [ ] **Step 3: Full gate and commit**

```bash
cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
git add crates/oregon-consensus/src/coinbase.rs crates/oregon-consensus/src/lib.rs
git commit -m "feat: enforce Oregon coinbase and founder grant rules"
```

Reviewer gate: founder mint only height 1/output 0, fees are miner-claimable, no maturity enforcement in M1.

---

### Task 4: Exact Oregon ASERT

**Files:** `src/asert.rs`, `src/lib.rs`.

**Produces:**

```rust
pub fn required_target(
    height: u64,
    parent_timestamp: u64,
    genesis_timestamp: u64,
    params: &ConsensusParams,
) -> Result<Target, ConsensusError>;
```

- [ ] **Step 1: Add deterministic RED vectors**

Use `initial_target = 1_000_000`, `pow_limit = 10_000_000`, genesis `1_800_000_000`:

```rust
#[test] fn h1_is_initial() { assert_eq!(required_target(1, 0, 0, &params()).unwrap(), target(1_000_000)); }
#[test] fn on_schedule_is_unchanged() { assert_eq!(required_target(2, G+300, G, &params()).unwrap(), target(1_000_000)); }
#[test] fn one_half_life_late_doubles() { assert_eq!(required_target(2, G+21_900, G, &params()).unwrap(), target(2_000_000)); }
#[test] fn one_half_life_early_halves() { assert_eq!(required_target(2, G-21_300, G, &params()).unwrap(), target(500_000)); }
#[test] fn half_half_life_late_is_frozen() { assert_eq!(required_target(2, G+11_100, G, &params()).unwrap(), target(1_414_093)); }
```

Also test huge positive exponent clamps to `pow_limit`; huge negative exponent clamps to target 1.

Run:

```bash
cargo +1.85.0 test -p oregon-consensus asert::tests --no-fail-fast
```

- [ ] **Step 2: Implement exact fixed-point arithmetic**

```rust
pub fn required_target(
    height: u64,
    parent_timestamp: u64,
    genesis_timestamp: u64,
    params: &ConsensusParams,
) -> Result<Target, ConsensusError> {
    if height == 0 { return Err(ConsensusError::InvalidHeight); }
    if height == 1 { return Ok(params.initial_target); }

    let time_delta = i128::from(parent_timestamp) - i128::from(genesis_timestamp);
    let height_delta = i128::from(height - 2);
    let ideal = 300i128.checked_mul(height_delta + 1).ok_or(ConsensusError::ArithmeticOverflow)?;
    let exponent = (time_delta - ideal)
        .checked_mul(65_536).ok_or(ConsensusError::ArithmeticOverflow)? / 21_600;
    let num_shifts = exponent >> 16;
    let frac = exponent - num_shifts * 65_536;
    if !(0..65_536).contains(&frac) { return Err(ConsensusError::ArithmeticOverflow); }

    let term1 = 195_766_423_245_049i128.checked_mul(frac).ok_or(ConsensusError::ArithmeticOverflow)?;
    let frac2 = frac.checked_mul(frac).ok_or(ConsensusError::ArithmeticOverflow)?;
    let frac3 = frac2.checked_mul(frac).ok_or(ConsensusError::ArithmeticOverflow)?;
    let term2 = 971_821_376i128.checked_mul(frac2).ok_or(ConsensusError::ArithmeticOverflow)?;
    let term3 = 5_127i128.checked_mul(frac3).ok_or(ConsensusError::ArithmeticOverflow)?;
    let polynomial = term1.checked_add(term2)
        .and_then(|v| v.checked_add(term3))
        .and_then(|v| v.checked_add(1i128 << 47))
        .ok_or(ConsensusError::ArithmeticOverflow)?;
    let factor = u64::try_from((polynomial >> 48) + 65_536)
        .map_err(|_| ConsensusError::ArithmeticOverflow)?;

    if num_shifts >= 256 { return Ok(params.pow_limit); }
    if num_shifts <= -257 { return Target::from_biguint(&BigUint::from(1u8)); }

    let mut candidate = params.initial_target.to_biguint() * BigUint::from(factor);
    if num_shifts < 0 {
        candidate >>= usize::try_from(-num_shifts).map_err(|_| ConsensusError::ArithmeticOverflow)?;
    } else {
        candidate <<= usize::try_from(num_shifts).map_err(|_| ConsensusError::ArithmeticOverflow)?;
    }
    candidate >>= 16usize;

    if candidate == BigUint::from(0u8) { return Target::from_biguint(&BigUint::from(1u8)); }
    if candidate > params.pow_limit.to_biguint() { return Ok(params.pow_limit); }
    Target::from_biguint(&candidate)
}
```

Rust signed division `/` supplies the spec's truncation-toward-zero rule; `i128 >> 16` supplies arithmetic right shift.

- [ ] **Step 3: Add properties**

```rust
proptest! {
    #[test]
    fn on_schedule_keeps_initial_target(height in 2u64..100_000) {
        let g = 1_800_000_000u64;
        let parent = g + 300 * (height - 1);
        prop_assert_eq!(required_target(height, parent, g, &params()).unwrap(), target(1_000_000));
    }
}
```

Add a bounded random property that every returned target is nonzero and `<= pow_limit` by numeric `BigUint` comparison.

- [ ] **Step 4: Full gate and commit**

```bash
cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
git add crates/oregon-consensus/src/asert.rs crates/oregon-consensus/src/lib.rs
git commit -m "feat: implement Oregon per-block ASERT"
```

Reviewer gate: h2 off-by-one, negative exponent semantics, coefficient identity, clamp proof, no float.

---

### Task 5: MTP, Chain Work, and Pre-PoW Header Context

**Files:** `src/time.rs`, `src/work.rs`, `src/header.rs`, `src/lib.rs`.

**Produces:**

```rust
pub fn median_time_past(window: &[u64]) -> Result<u64, ConsensusError>;
pub struct ChainWork(BigUint);
pub fn block_work(target: Target) -> ChainWork;
pub struct HeaderContext<'a> {
    pub height: u64,
    pub parent: &'a BlockHeader,
    pub genesis_timestamp: u64,
    pub mtp_window: &'a [u64],
}
pub struct PrePowHeaderFacts { pub target: Target, pub work: ChainWork }
pub fn validate_header_pre_pow(...) -> Result<PrePowHeaderFacts, ConsensusError>;
```

- [ ] **Step 1: Add RED MTP/work tests**

```rust
#[test] fn even_early_mtp_uses_upper_median() { assert_eq!(median_time_past(&[100, 200]).unwrap(), 200); }
#[test] fn empty_or_twelve_item_window_is_invalid() {
    assert_eq!(median_time_past(&[]), Err(ConsensusError::InvalidMtpWindow));
    assert_eq!(median_time_past(&[0; 12]), Err(ConsensusError::InvalidMtpWindow));
}
#[test] fn max_target_has_one_work_unit() {
    let t = Target::from_le_bytes([0xff; 32]).unwrap();
    assert_eq!(block_work(t).to_biguint(), BigUint::from(1u8));
}
#[test] fn target_one_has_two_to_255_work() {
    let t = Target::from_biguint(&BigUint::from(1u8)).unwrap();
    assert_eq!(block_work(t).to_biguint(), BigUint::from(1u8) << 255usize);
}
```

- [ ] **Step 2: Add RED header-context cases**

Construct parent/child headers and prove wrong parent, timestamp `<= MTP`, zero/above-limit target, and wrong expected target fail. Valid context returns work facts without invoking RandomX.

Run:

```bash
cargo +1.85.0 test -p oregon-consensus time::tests work::tests header::tests --no-fail-fast
```

- [ ] **Step 3: Implement MTP and work**

```rust
pub fn median_time_past(window: &[u64]) -> Result<u64, ConsensusError> {
    if window.is_empty() || window.len() > 11 { return Err(ConsensusError::InvalidMtpWindow); }
    let mut sorted = window.to_vec();
    sorted.sort_unstable();
    Ok(sorted[sorted.len() / 2])
}
```

```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChainWork(BigUint);

impl ChainWork {
    pub fn zero() -> Self { Self(BigUint::from(0u8)) }
    pub fn to_biguint(&self) -> BigUint { self.0.clone() }
    pub fn add_assign(&mut self, rhs: &Self) { self.0 += &rhs.0; }
}

pub fn block_work(target: Target) -> ChainWork {
    let numerator = BigUint::from(1u8) << 256usize;
    let denominator = target.to_biguint() + BigUint::from(1u8);
    ChainWork(numerator / denominator)
}
```

- [ ] **Step 4: Implement pre-PoW header context**

```rust
pub fn validate_header_pre_pow(
    header: &BlockHeader,
    context: &HeaderContext<'_>,
    params: &ConsensusParams,
) -> Result<PrePowHeaderFacts, ConsensusError> {
    if context.height == 0 { return Err(ConsensusError::InvalidHeight); }
    if header.previous_block != context.parent.block_id() {
        return Err(ConsensusError::PreviousBlockMismatch);
    }
    let mtp = median_time_past(context.mtp_window)?;
    if header.timestamp <= mtp { return Err(ConsensusError::TimestampNotAfterMtp); }

    let expected = required_target(
        context.height,
        context.parent.timestamp,
        context.genesis_timestamp,
        params,
    )?;
    let actual = Target::from_le_bytes(header.difficulty_commitment)?;
    actual.validate_against(params.pow_limit)?;
    if actual != expected { return Err(ConsensusError::UnexpectedTarget); }

    Ok(PrePowHeaderFacts { target: actual, work: block_work(actual) })
}
```

- [ ] **Step 5: Full gate and commit**

```bash
cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
git add crates/oregon-consensus/src/time.rs crates/oregon-consensus/src/work.rs crates/oregon-consensus/src/header.rs crates/oregon-consensus/src/lib.rs
git commit -m "feat: validate Oregon header context before PoW"
```

Reviewer gate: parent timestamp feeds ASERT, candidate timestamp only MTP floor, exact work formula, API cannot be mistaken for PoW verification.

---

### Task 6: Non-Genesis Block Structural Validation

**Files:** `src/block.rs`, `src/lib.rs`.

**Produces:**

```rust
pub fn validate_non_genesis_block_structure(
    block: &Block,
    height: u64,
    total_fees: Amount,
    params: &ConsensusParams,
) -> Result<(), ConsensusError>;
```

`total_fees` is supplied by M3's future state transition. M1 does not validate UTXO existence, signatures, double spends, maturity, or fees.

- [ ] **Step 1: Add RED structural tests**

Prove:

```rust
#[test] fn valid_small_block_passes() { /* valid coinbase + matching Merkle -> Ok */ }
#[test] fn changed_merkle_root_fails() { /* -> MerkleRootMismatch */ }
#[test] fn second_coinbase_form_fails() { /* -> MultipleCoinbase */ }
#[test] fn normal_transaction_null_outpoint_fails() { /* -> NullOutpointInNormalTransaction */ }
#[test] fn tx_over_102400_bytes_fails() { /* -> TransactionTooLarge(index) */ }
#[test] fn block_over_1048576_bytes_fails() { /* -> BlockTooLarge */ }
```

Run RED:

```bash
cargo +1.85.0 test -p oregon-consensus block::tests --no-fail-fast
```

- [ ] **Step 2: Implement cheap-first structure validation**

```rust
pub fn validate_non_genesis_block_structure(
    block: &Block,
    height: u64,
    total_fees: Amount,
    params: &ConsensusParams,
) -> Result<(), ConsensusError> {
    if height == 0 { return Err(ConsensusError::InvalidHeight); }
    if block.encode().len() > MAX_BLOCK_BYTES { return Err(ConsensusError::BlockTooLarge); }
    if block.transactions.is_empty() { return Err(ConsensusError::EmptyNonGenesisBlock); }

    for (index, tx) in block.transactions.iter().enumerate() {
        if tx.encode().len() > MAX_TRANSACTION_BYTES {
            return Err(ConsensusError::TransactionTooLarge(index));
        }
    }

    let root = transaction_root(&block.transactions).map_err(|_| ConsensusError::MerkleRootMismatch)?;
    if root != block.header.transaction_root { return Err(ConsensusError::MerkleRootMismatch); }

    validate_coinbase(&block.transactions[0], height, total_fees, params)?;

    for tx in &block.transactions[1..] {
        if is_coinbase_form(tx) { return Err(ConsensusError::MultipleCoinbase); }
        if tx.inputs.iter().any(|input| {
            input.previous_txid == Hash256::from_bytes([0u8; 32])
                && input.previous_output_index == u32::MAX
        }) {
            return Err(ConsensusError::NullOutpointInNormalTransaction);
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Full gate and commit**

```bash
cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
git add crates/oregon-consensus/src/block.rs crates/oregon-consensus/src/lib.rs
git commit -m "feat: enforce Oregon non-genesis block structure"
```

Reviewer gate: 1 MiB before expensive body work, 100 KiB includes coinbase, frozen v0 Merkle algorithm reused, no accidental M2/M3 behavior.

---

### Task 7: Golden Vectors, Mutation Sensitivity, and M1 Acceptance

**Files:** `tests/vectors/consensus-m1-v1.json`, `crates/oregon-consensus/tests/golden_vectors.rs`, `docs/checkpoints/OREGON_V1_M1_CONSENSUS_CORE.md`, `docs/checkpoints/OREGON_V0_PROTOCOL_PROGRESS.md`.

- [ ] **Step 1: Write golden consumer first, with fixture absent**

The consumer must rebuild targets/emission/ASERT/work through public APIs and read `tests/vectors/consensus-m1-v1.json`. Run:

```bash
cargo +1.85.0 test -p oregon-consensus --test golden_vectors
```

Expected: failure specifically because the fixture file is absent.

- [ ] **Step 2: Add exact fixture**

```json
{
  "target": {
    "little_endian_hex": "3412000000000000000000000000000000000000000000000000000000000000",
    "integer_decimal": "4660"
  },
  "emission": {
    "height_1": 237500000,
    "height_200000": 237500000,
    "height_200001": 118750000,
    "era_27_first": 1,
    "era_28_first": 0,
    "scheduled_mining": 94999997000000,
    "scheduled_with_founder": 99999997000000
  },
  "asert": [
    {"name":"on_schedule", "height":2, "parent_delta":300, "expected_target":"1000000"},
    {"name":"half_life_late", "height":2, "parent_delta":21900, "expected_target":"2000000"},
    {"name":"half_life_early", "height":2, "parent_delta":-21300, "expected_target":"500000"},
    {"name":"half_half_life_late", "height":2, "parent_delta":11100, "expected_target":"1414093"}
  ],
  "work": {
    "target_max_work": "1",
    "target_one_work_hex": "8000000000000000000000000000000000000000000000000000000000000000"
  }
}
```

Use genesis timestamp `1_800_000_000`, initial target `1_000_000`, pow limit `10_000_000` in the ASERT consumer. Run the golden test and require PASS.

- [ ] **Step 3: Mutation A — emission off-by-one must be caught**

On throwaway branch `oregon-v1-m1-mutation-emission-asert`, change only:

```rust
let era = (height - 1) / HALVING_INTERVAL;
```

to:

```rust
let era = height / HALVING_INTERVAL;
```

Run:

```bash
cargo +1.85.0 test -p oregon-consensus emission::tests --test golden_vectors
```

Expected: failure at a halving boundary and/or golden emission vector. Record exact failing test and run/commit evidence outside the clean M1 branch, then revert the mutation.

- [ ] **Step 4: Mutation B — ASERT half-life mutation must be caught**

On the same throwaway branch, change implementation half-life from `21_600` to `21_601` while tests/vectors remain unchanged. Run:

```bash
cargo +1.85.0 test -p oregon-consensus asert::tests --test golden_vectors
```

Expected: at least one non-zero-exponent vector fails. Record exact evidence. Do not merge mutation commits.

- [ ] **Step 5: Clean M1 exact-head acceptance gate**

```bash
git switch oregon-v1-m1-consensus-core
cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
```

Require all commands exit 0.

- [ ] **Step 6: Create checkpoint using observed evidence values, not a blank template**

After the successful final code commit exists:

```bash
ACCEPTED_SHA="$(git rev-parse HEAD)"
printf '%s\n' "$ACCEPTED_SHA"
```

Read the completed GitHub Actions run for exactly that SHA and set `CI_RUN_ID` to that observed decimal run ID. Abort checkpoint creation if the run is absent, pending, or not fully successful.

Then write `docs/checkpoints/OREGON_V1_M1_CONSENSUS_CORE.md` with the concrete values already obtained. The document must name:

- accepted SHA from `ACCEPTED_SHA`;
- successful exact-head `CI_RUN_ID`;
- exact mutation branch/commit/run evidence from Steps 3–4;
- golden vector path;
- accepted features: target, emission, coinbase/founder, ASERT, MTP, work, pre-PoW header context, block structural rules;
- excluded features: RandomX/M2, UTXO/signatures/maturity/M3, storage/M4, genesis/address/M5, networking and later subsystems.

A checkpoint is invalid if any evidence field is missing or describes a different SHA.

- [ ] **Step 7: Append v0 progress pointer**

Append exactly:

```markdown
## Next milestone

Oregon v1 M1 Consensus Core was developed separately from the frozen v0 foundation. See `docs/checkpoints/OREGON_V1_M1_CONSENSUS_CORE.md` for its acceptance evidence. The v0 foundation checkpoint remains unchanged and recoverable.
```

- [ ] **Step 8: Commit vectors/checkpoint, then verify that exact docs head again**

```bash
git add tests/vectors/consensus-m1-v1.json \
  crates/oregon-consensus/tests/golden_vectors.rs \
  docs/checkpoints/OREGON_V1_M1_CONSENSUS_CORE.md \
  docs/checkpoints/OREGON_V0_PROTOCOL_PROGRESS.md
git commit -m "test: freeze Oregon M1 consensus vectors and checkpoint"

cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
```

For remote CI, require a completed successful run whose `head_sha` equals this final docs commit.

- [ ] **Step 9: Create recovery branch only from that exact successful head**

```bash
git branch oregon-v1-checkpoint-m1-consensus-core-accepted-2026-09-03
git push origin oregon-v1-checkpoint-m1-consensus-core-accepted-2026-09-03
```

Verify branch SHA equals the exact successful final head.

Reviewer gate: compare M1 against master spec Sections 4, 6, 7, 8, 9, 10, 13, 18, and 20. A removed target/subsidy/founder/ASERT/MTP/work/size/Merkle check whose tests remain green is a failed milestone.

---

## Plan Self-Review Record

**Spec coverage:** target/params -> Task 1; emission -> Task 2; coinbase/founder/reward -> Task 3; ASERT -> Task 4; MTP/work/header context -> Task 5; block size/Merkle/coinbase uniqueness -> Task 6; vectors/mutations/acceptance -> Task 7.

**Type consistency:** all difficulty APIs use `Target`; `ConsensusParams` owns launch-profile target/founder inputs; amounts crossing primitives/consensus use `Amount`; ASERT returns `Target`; chain work uses `BigUint` through `ChainWork`; block structure receives precomputed `total_fees: Amount` rather than pretending to validate UTXOs.

**Scope exclusions:** RandomX M2; KeyCommit/BIP340/UTXO/maturity M3; persistence/reorg M4; address/genesis/network profile M5; P2P M6; mempool/mining/RPC M7; benchmark/mainnet freeze M8.

**Acceptance principle:** M1 is accepted only when the exact final head passes workspace test/format/clippy and both deliberate emission/ASERT mutations are detected by tests. If either mutation leaves the relevant suite green, acceptance stops.