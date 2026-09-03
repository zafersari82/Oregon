# Oregon M1 Consensus Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first Oregon v1 consensus crate with exact target arithmetic, issuance, one-time founder/coinbase rules, ASERT, median-time-past, chain-work calculation, pre-PoW header-context validation, and non-genesis block structural validation.

**Architecture:** Add a focused `oregon-consensus` crate above the frozen `oregon-primitives` crate. Wire-format target bytes remain exactly the existing 32-byte `BlockHeader::difficulty_commitment`; consensus converts those bytes into a `Target` value and uses exact big-integer arithmetic for ASERT and work calculations. M1 deliberately stops before RandomX, UTXO/signature validation, persistence, P2P, and genesis generation so every rule in this milestone can be reviewed independently.

**Tech Stack:** Rust 1.85.0, edition 2024, `oregon-primitives`, `thiserror`, exact `num-bigint = 0.5.1`, exact `num-traits = 0.2.19`, `proptest`/`serde`/`serde_json` for tests only.

**Spec:** `docs/superpowers/specs/2026-09-02-oregon-pow-consensus-v1-design.md`

## Global Constraints

- Base implementation checkpoint: `033160dafd4c2a74cd6dcfa2bb7b628c3cab499c` plus approved v1 design branch history through `a087a1ec66bd4f114482e56ced78b4747da108e7`.
- Execution branch: `oregon-v1-m1-consensus-core`, created from the approved plan/spec head.
- Rust edition 2024; MSRV 1.85.0.
- No floating-point value may influence consensus.
- No `unsafe` in Oregon consensus code.
- No Bitcoin source imports, `.cpp/.h` copying, bincode, postcard, or generic binary consensus codec.
- Canonical header format stays 114 bytes; M1 does not change any v0 primitive encoding or golden vector.
- Target byte order is exactly unsigned little-endian `[u8; 32]`.
- Target range is `1..=POW_LIMIT`.
- Height 1 target is exactly `INITIAL_TARGET`.
- For height `h >= 2`, ASERT uses fixed anchor target `INITIAL_TARGET`, genesis timestamp as anchor-parent time, parent timestamp as evaluation time, target interval 300 seconds, half-life 21,600 seconds, radix 65,536, and the polynomial coefficients frozen by the v1 design.
- Genesis creates no OREG and is outside M1 block validation; M1 validates non-genesis blocks only.
- Initial subsidy is `237,500,000` base units; halving interval is `200,000`; scheduled mining issuance is exactly `94,999,997,000,000` base units.
- Height 1 founder output is exactly index 0, exactly `5,000,000,000,000` base units, locking program `0x01 || FOUNDER_KEY_COMMITMENT`.
- Coinbase maturity is specified by the master design but enforced in M3, not M1, because M1 has no UTXO state.
- Maximum canonical non-genesis block size is 1,048,576 bytes; every transaction in a non-genesis block, including coinbase, is at most 102,400 bytes.
- MTP window is 1..=11 parent/ancestor timestamps, sorted internally; zero-based `floor(count/2)` selects the median.
- M1 header validation is explicitly **pre-PoW**. It MUST NOT claim RandomX verification. `oregon-pow` is M2.
- Commit `Cargo.lock` once M1 dependencies are resolved so CI uses a reproducible dependency graph.
- Every task follows RED -> GREEN -> full gate -> reviewer gate -> commit. Do not continue when a Critical or Important review issue remains open.

---

## File Map

### New crate

- `crates/oregon-consensus/Cargo.toml` — crate dependencies only.
- `crates/oregon-consensus/src/lib.rs` — public exports; no implementation logic.
- `crates/oregon-consensus/src/error.rs` — typed M1 consensus errors.
- `crates/oregon-consensus/src/params.rs` — `ConsensusParams` and fixed M1 constants.
- `crates/oregon-consensus/src/target.rs` — `Target`, exact LE conversion, target bounds.
- `crates/oregon-consensus/src/emission.rs` — subsidy and scheduled issuance.
- `crates/oregon-consensus/src/coinbase.rs` — non-genesis coinbase structure/founder/reward ceiling.
- `crates/oregon-consensus/src/asert.rs` — exact Oregon ASERT target calculation.
- `crates/oregon-consensus/src/time.rs` — MTP calculation.
- `crates/oregon-consensus/src/work.rs` — per-block/cumulative work value.
- `crates/oregon-consensus/src/header.rs` — parent linkage, time, expected target; explicitly pre-PoW.
- `crates/oregon-consensus/src/block.rs` — 1 MiB/100 KiB, Merkle, unique coinbase and reward integration.
- `crates/oregon-consensus/tests/golden_vectors.rs` — frozen M1 external vectors.

### New protocol artifact

- `tests/vectors/consensus-m1-v1.json` — target/ASERT/work/emission/coinbase deterministic vectors.

### Modified existing files

- `Cargo.toml` — add `crates/oregon-consensus` workspace member.
- `.github/workflows/oregon-rust.yml` — run on `oregon-v1-m1-consensus-core` pushes in addition to the existing v0 branch.
- `docs/checkpoints/OREGON_V1_M1_CONSENSUS_CORE.md` — final accepted checkpoint evidence.
- `docs/checkpoints/OREGON_V0_PROTOCOL_PROGRESS.md` — append next-milestone pointer only after M1 acceptance; do not rewrite historical v0 evidence.
- `Cargo.lock` — generated and committed after dependencies resolve.

---

### Task 1: Consensus Crate, Parameters, and Full-Width Target

**Files:**
- Modify: `Cargo.toml`
- Modify: `.github/workflows/oregon-rust.yml`
- Create: `crates/oregon-consensus/Cargo.toml`
- Create: `crates/oregon-consensus/src/lib.rs`
- Create: `crates/oregon-consensus/src/error.rs`
- Create: `crates/oregon-consensus/src/params.rs`
- Create: `crates/oregon-consensus/src/target.rs`
- Generate: `Cargo.lock`

**Interfaces:**
- Consumes: `oregon_primitives::BlockHeader` target bytes later; no header validation yet.
- Produces:
  - `pub struct Target([u8; 32]);`
  - `Target::from_le_bytes([u8; 32]) -> Result<Target, ConsensusError>`
  - `Target::to_le_bytes(self) -> [u8; 32]`
  - `Target::to_biguint(&self) -> BigUint`
  - `Target::from_biguint(&BigUint) -> Result<Target, ConsensusError>`
  - `Target::validate_against(self, pow_limit: Target) -> Result<(), ConsensusError>`
  - `pub struct ConsensusParams { pub pow_limit: Target, pub initial_target: Target, pub founder_key_commitment: [u8;32] }`
  - `ConsensusParams::new(...) -> Result<Self, ConsensusError>`

- [ ] **Step 1: Create the M1 execution branch from the approved plan/spec head**

Run:

```bash
git switch oregon-v1-consensus-design
git pull --ff-only
git switch -c oregon-v1-m1-consensus-core
```

Expected: new branch points at the plan/spec head and contains no production M1 code yet.

- [ ] **Step 2: Add failing target/parameter tests first**

Create `crates/oregon-consensus/src/target.rs` with tests referencing not-yet-implemented functions:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_target_is_invalid() {
        assert_eq!(
            Target::from_le_bytes([0u8; 32]),
            Err(ConsensusError::ZeroTarget)
        );
    }

    #[test]
    fn little_endian_target_round_trips_exactly() {
        let mut bytes = [0u8; 32];
        bytes[0] = 0x34;
        bytes[1] = 0x12;
        let target = Target::from_le_bytes(bytes).unwrap();
        assert_eq!(target.to_le_bytes(), bytes);
        assert_eq!(target.to_biguint(), BigUint::from(0x1234u32));
    }

    #[test]
    fn value_larger_than_256_bits_is_rejected() {
        let too_large = BigUint::from(1u8) << 256usize;
        assert_eq!(
            Target::from_biguint(&too_large),
            Err(ConsensusError::TargetExceeds256Bits)
        );
    }

    #[test]
    fn initial_target_must_not_exceed_pow_limit() {
        let pow_limit = Target::from_biguint(&BigUint::from(100u32)).unwrap();
        let initial = Target::from_biguint(&BigUint::from(101u32)).unwrap();
        assert_eq!(
            ConsensusParams::new(pow_limit, initial, [7u8; 32]),
            Err(ConsensusError::InitialTargetAbovePowLimit)
        );
    }
}
```

- [ ] **Step 3: Wire the crate minimally and run RED**

Root `Cargo.toml` becomes:

```toml
[workspace]
members = ["crates/oregon-primitives", "crates/oregon-consensus"]
resolver = "3"

[workspace.package]
edition = "2024"
rust-version = "1.85.0"
```

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

Run:

```bash
cargo +1.85.0 test -p oregon-consensus target::tests --no-fail-fast
```

Expected: FAIL because `Target`, `ConsensusParams`, and/or `ConsensusError` are incomplete.

- [ ] **Step 4: Implement the typed errors and exact target type**

`error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConsensusError {
    #[error("target must be non-zero")]
    ZeroTarget,
    #[error("target exceeds 256 bits")]
    TargetExceeds256Bits,
    #[error("target exceeds POW_LIMIT")]
    TargetAbovePowLimit,
    #[error("INITIAL_TARGET exceeds POW_LIMIT")]
    InitialTargetAbovePowLimit,
    #[error("consensus arithmetic overflow")]
    ArithmeticOverflow,
    #[error("invalid non-genesis height")]
    InvalidHeight,
    #[error("unexpected difficulty target")]
    UnexpectedTarget,
    #[error("invalid median-time-past window")]
    InvalidMtpWindow,
    #[error("block timestamp is not greater than median-time-past")]
    TimestampNotAfterMtp,
    #[error("previous block id does not match parent")]
    PreviousBlockMismatch,
    #[error("coinbase structure is invalid")]
    InvalidCoinbase,
    #[error("height-1 founder output is invalid")]
    InvalidFounderOutput,
    #[error("coinbase claims more than subsidy plus fees")]
    CoinbaseOverClaim,
    #[error("block exceeds the v1 canonical byte limit")]
    BlockTooLarge,
    #[error("transaction at index {0} exceeds the v1 canonical byte limit")]
    TransactionTooLarge(usize),
    #[error("non-genesis block has no transactions")]
    EmptyNonGenesisBlock,
    #[error("block transaction root does not match header")]
    MerkleRootMismatch,
    #[error("a non-coinbase transaction uses the null outpoint")]
    NullOutpointInNormalTransaction,
    #[error("multiple coinbase-form transactions appear in one block")]
    MultipleCoinbase,
}
```

`target.rs` core implementation:

```rust
use num_bigint::BigUint;
use num_traits::Zero;

use crate::ConsensusError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Target([u8; 32]);

impl Target {
    pub fn from_le_bytes(bytes: [u8; 32]) -> Result<Self, ConsensusError> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(ConsensusError::ZeroTarget);
        }
        Ok(Self(bytes))
    }

    pub const fn to_le_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn to_biguint(&self) -> BigUint {
        BigUint::from_bytes_le(&self.0)
    }

    pub fn from_biguint(value: &BigUint) -> Result<Self, ConsensusError> {
        if value.is_zero() {
            return Err(ConsensusError::ZeroTarget);
        }
        let bytes = value.to_bytes_le();
        if bytes.len() > 32 {
            return Err(ConsensusError::TargetExceeds256Bits);
        }
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
    pub fn new(
        pow_limit: Target,
        initial_target: Target,
        founder_key_commitment: [u8; 32],
    ) -> Result<Self, ConsensusError> {
        if initial_target.to_biguint() > pow_limit.to_biguint() {
            return Err(ConsensusError::InitialTargetAbovePowLimit);
        }
        Ok(Self { pow_limit, initial_target, founder_key_commitment })
    }
}
```

`lib.rs` exports only named modules/types:

```rust
pub mod asert;
pub mod block;
pub mod coinbase;
pub mod emission;
pub mod error;
pub mod header;
pub mod params;
pub mod target;
pub mod time;
pub mod work;

pub use error::ConsensusError;
pub use params::ConsensusParams;
pub use target::Target;
```

For modules not implemented in Task 1, create empty files containing only a module-level comment such as `//! Implemented by a later M1 task.`; do not add stub functions that could be mistaken for working consensus.

- [ ] **Step 5: Enable CI for the M1 branch and generate the lockfile**

Change the workflow push filter to:

```yaml
on:
  push:
    branches: [oregon-v0-protocol, oregon-v1-m1-consensus-core]
  pull_request:
    branches: [main]
```

Run:

```bash
cargo +1.85.0 generate-lockfile
cargo +1.85.0 test -p oregon-consensus target::tests
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy -p oregon-consensus --all-targets -- -D warnings
```

Expected: all target/params tests pass; formatting and clippy succeed.

- [ ] **Step 6: Full workspace gate and commit**

Run:

```bash
cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
```

Then commit:

```bash
git add Cargo.toml Cargo.lock .github/workflows/oregon-rust.yml crates/oregon-consensus
git commit -m "feat: establish Oregon consensus target model"
```

Reviewer gate: verify `Target` byte order, zero/overflow rejection, `INITIAL_TARGET <= POW_LIMIT`, exact dependency versions in lockfile, and no v0 primitive format change.

---

### Task 2: Exact Mining Emission Schedule

**Files:**
- Modify: `crates/oregon-consensus/src/emission.rs`
- Modify: `crates/oregon-consensus/src/lib.rs`

**Interfaces:**
- Consumes: `Amount`, `HALVING_INTERVAL`, `INITIAL_SUBSIDY_BASE_UNITS`.
- Produces:
  - `pub fn block_subsidy(height: u64) -> Result<Amount, ConsensusError>`
  - `pub const SCHEDULED_MINING_ISSUANCE_BASE_UNITS: u64 = 94_999_997_000_000`
  - `pub const SCHEDULED_TOTAL_WITH_FOUNDER_BASE_UNITS: u64 = 99_999_997_000_000`

- [ ] **Step 1: Write boundary and total-supply tests before implementation**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use oregon_primitives::{FOUNDER_ALLOCATION_BASE_UNITS, MAX_SUPPLY_BASE_UNITS};

    #[test]
    fn height_zero_has_no_mining_subsidy() {
        assert_eq!(block_subsidy(0).unwrap().base_units(), 0);
    }

    #[test]
    fn subsidy_halves_on_exact_boundaries() {
        assert_eq!(block_subsidy(1).unwrap().base_units(), 237_500_000);
        assert_eq!(block_subsidy(200_000).unwrap().base_units(), 237_500_000);
        assert_eq!(block_subsidy(200_001).unwrap().base_units(), 118_750_000);
    }

    #[test]
    fn era_27_is_last_positive_era() {
        let start_27 = 27 * HALVING_INTERVAL + 1;
        let start_28 = 28 * HALVING_INTERVAL + 1;
        assert_eq!(block_subsidy(start_27).unwrap().base_units(), 1);
        assert_eq!(block_subsidy(start_28).unwrap().base_units(), 0);
    }

    #[test]
    fn scheduled_issuance_is_exact_and_never_tops_up() {
        let mut total = 0u128;
        for era in 0..28u64 {
            let first_height = era * HALVING_INTERVAL + 1;
            total += u128::from(block_subsidy(first_height).unwrap().base_units())
                * u128::from(HALVING_INTERVAL);
        }
        assert_eq!(total, 94_999_997_000_000u128);
        assert_eq!(SCHEDULED_MINING_ISSUANCE_BASE_UNITS, total as u64);
        assert_eq!(
            SCHEDULED_TOTAL_WITH_FOUNDER_BASE_UNITS,
            SCHEDULED_MINING_ISSUANCE_BASE_UNITS + FOUNDER_ALLOCATION_BASE_UNITS
        );
        assert_eq!(MAX_SUPPLY_BASE_UNITS - SCHEDULED_TOTAL_WITH_FOUNDER_BASE_UNITS, 3_000_000);
    }
}
```

- [ ] **Step 2: Run RED**

```bash
cargo +1.85.0 test -p oregon-consensus emission::tests --no-fail-fast
```

Expected: FAIL because emission functions/constants are absent.

- [ ] **Step 3: Implement integer-only emission**

```rust
use oregon_primitives::{Amount, FOUNDER_ALLOCATION_BASE_UNITS};

use crate::params::{HALVING_INTERVAL, INITIAL_SUBSIDY_BASE_UNITS};
use crate::ConsensusError;

pub const SCHEDULED_MINING_ISSUANCE_BASE_UNITS: u64 = 94_999_997_000_000;
pub const SCHEDULED_TOTAL_WITH_FOUNDER_BASE_UNITS: u64 =
    SCHEDULED_MINING_ISSUANCE_BASE_UNITS + FOUNDER_ALLOCATION_BASE_UNITS;

pub fn block_subsidy(height: u64) -> Result<Amount, ConsensusError> {
    if height == 0 {
        return Amount::from_base_units(0).map_err(|_| ConsensusError::ArithmeticOverflow);
    }
    let era = (height - 1) / HALVING_INTERVAL;
    let subsidy = if era >= 64 {
        0
    } else {
        INITIAL_SUBSIDY_BASE_UNITS >> (era as u32)
    };
    Amount::from_base_units(subsidy).map_err(|_| ConsensusError::ArithmeticOverflow)
}
```

- [ ] **Step 4: Run focused and full gates**

```bash
cargo +1.85.0 test -p oregon-consensus emission::tests
cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
```

Expected: exact issuance and all prior tests pass.

- [ ] **Step 5: Commit and review**

```bash
git add crates/oregon-consensus/src/emission.rs crates/oregon-consensus/src/lib.rs
git commit -m "feat: define Oregon mining emission schedule"
```

Reviewer gate: manually verify height math uses `(h-1)/200000`, era 27 produces one base unit, era 28 produces zero, and no top-up path exists.

---

### Task 3: Coinbase Structure, Founder Grant, and Reward Ceiling

**Files:**
- Modify: `crates/oregon-consensus/src/coinbase.rs`
- Modify: `crates/oregon-consensus/src/lib.rs`

**Interfaces:**
- Consumes: `Transaction`, `Amount`, `write_varint`, `ConsensusParams`, `block_subsidy`.
- Produces:
  - `pub fn is_coinbase_form(tx: &Transaction) -> bool`
  - `pub fn validate_coinbase(tx: &Transaction, height: u64, total_fees: Amount, params: &ConsensusParams) -> Result<(), ConsensusError>`

- [ ] **Step 1: Add a reusable test coinbase constructor and failing founder tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use oregon_primitives::{Hash256, TxInput, TxOutput};

    fn params() -> ConsensusParams {
        ConsensusParams::new(
            Target::from_biguint(&1000u32.into()).unwrap(),
            Target::from_biguint(&500u32.into()).unwrap(),
            [0x42; 32],
        ).unwrap()
    }

    fn coinbase(height: u64, outputs: Vec<TxOutput>) -> Transaction {
        let mut height_bytes = Vec::new();
        write_varint(height, &mut height_bytes);
        Transaction {
            version: 1,
            inputs: vec![TxInput {
                previous_txid: Hash256::from_bytes([0u8; 32]),
                previous_output_index: u32::MAX,
                sequence: u32::MAX,
                witness: vec![height_bytes],
            }],
            outputs,
            lock_time: 0,
        }
    }

    #[test]
    fn height_one_requires_exact_founder_output_at_index_zero() {
        let mut founder_program = vec![KEY_COMMIT_V1];
        founder_program.extend_from_slice(&[0x42; 32]);
        let tx = coinbase(1, vec![TxOutput {
            value: Amount::from_base_units(5_000_000_000_000).unwrap(),
            locking_program: founder_program,
        }]);
        assert_eq!(
            validate_coinbase(&tx, 1, Amount::from_base_units(0).unwrap(), &params()),
            Ok(())
        );
    }

    #[test]
    fn founder_value_mutation_is_rejected() {
        let mut founder_program = vec![KEY_COMMIT_V1];
        founder_program.extend_from_slice(&[0x42; 32]);
        let tx = coinbase(1, vec![TxOutput {
            value: Amount::from_base_units(4_999_999_999_999).unwrap(),
            locking_program: founder_program,
        }]);
        assert_eq!(
            validate_coinbase(&tx, 1, Amount::from_base_units(0).unwrap(), &params()),
            Err(ConsensusError::InvalidFounderOutput)
        );
    }

    #[test]
    fn height_two_has_no_special_founder_mint() {
        let over_claim = TxOutput {
            value: Amount::from_base_units(5_000_000_000_000).unwrap(),
            locking_program: vec![0x51],
        };
        assert_eq!(
            validate_coinbase(
                &coinbase(2, vec![over_claim]),
                2,
                Amount::from_base_units(0).unwrap(),
                &params()
            ),
            Err(ConsensusError::CoinbaseOverClaim)
        );
    }
}
```

- [ ] **Step 2: Add failing structure and over-claim tests**

Cover exact height witness, version/lock_time, one null input, `sequence = u32::MAX`, under-claim allowed, and `subsidy + fees + 1` rejected. Use canonical height bytes from `write_varint` and mutate one byte to prove exact matching.

Example over-claim test:

```rust
#[test]
fn miner_may_underclaim_but_never_overclaim() {
    let fees = Amount::from_base_units(10).unwrap();
    let allowed = block_subsidy(2).unwrap().base_units() + 10;
    let ok = coinbase(2, vec![TxOutput {
        value: Amount::from_base_units(allowed - 1).unwrap(),
        locking_program: vec![0x51],
    }]);
    assert!(validate_coinbase(&ok, 2, fees, &params()).is_ok());

    let bad = coinbase(2, vec![TxOutput {
        value: Amount::from_base_units(allowed + 1).unwrap(),
        locking_program: vec![0x51],
    }]);
    assert_eq!(
        validate_coinbase(&bad, 2, fees, &params()),
        Err(ConsensusError::CoinbaseOverClaim)
    );
}
```

- [ ] **Step 3: Run RED**

```bash
cargo +1.85.0 test -p oregon-consensus coinbase::tests --no-fail-fast
```

Expected: FAIL because coinbase validation is absent.

- [ ] **Step 4: Implement exact coinbase-form and founder/reward checks**

Core shape:

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
    if input.witness[0] != expected_height {
        return Err(ConsensusError::InvalidCoinbase);
    }

    let miner_start = if height == 1 {
        let founder = tx.outputs.first().ok_or(ConsensusError::InvalidFounderOutput)?;
        let mut expected_program = Vec::with_capacity(33);
        expected_program.push(KEY_COMMIT_V1);
        expected_program.extend_from_slice(&params.founder_key_commitment);
        if founder.value.base_units() != FOUNDER_ALLOCATION_BASE_UNITS
            || founder.locking_program != expected_program
        {
            return Err(ConsensusError::InvalidFounderOutput);
        }
        1
    } else {
        0
    };

    let miner_claim = tx.outputs[miner_start..].iter().try_fold(0u64, |sum, output| {
        sum.checked_add(output.value.base_units())
            .ok_or(ConsensusError::ArithmeticOverflow)
    })?;
    let ceiling = block_subsidy(height)?
        .base_units()
        .checked_add(total_fees.base_units())
        .ok_or(ConsensusError::ArithmeticOverflow)?;
    if miner_claim > ceiling {
        return Err(ConsensusError::CoinbaseOverClaim);
    }
    Ok(())
}
```

- [ ] **Step 5: Full gate and commit**

```bash
cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
git add crates/oregon-consensus/src/coinbase.rs crates/oregon-consensus/src/lib.rs
git commit -m "feat: enforce Oregon coinbase and founder grant rules"
```

Reviewer gate: confirm founder grant can occur only via height-1 output 0, miner reward starts at output 1 only on height 1, zero-output coinbase remains valid for heights >1 as full under-claim, and the function does not enforce M3 UTXO maturity prematurely.

---

### Task 4: Exact Oregon ASERT

**Files:**
- Modify: `crates/oregon-consensus/src/asert.rs`
- Modify: `crates/oregon-consensus/src/lib.rs`

**Interfaces:**
- Consumes: `Target`, `ConsensusParams`, fixed ASERT constants.
- Produces:
  - `pub fn required_target(height: u64, parent_timestamp: u64, genesis_timestamp: u64, params: &ConsensusParams) -> Result<Target, ConsensusError>`

- [ ] **Step 1: Add RED tests for height 1, on-schedule, full half-life early/late, half-half-life fraction, and clamps**

Use small integer targets so expected values are independent and readable:

```rust
fn target(value: u64) -> Target {
    Target::from_biguint(&BigUint::from(value)).unwrap()
}

fn params() -> ConsensusParams {
    ConsensusParams::new(target(10_000_000), target(1_000_000), [0u8; 32]).unwrap()
}

#[test]
fn height_one_is_exact_initial_target() {
    assert_eq!(required_target(1, 0, 0, &params()).unwrap(), target(1_000_000));
}

#[test]
fn on_schedule_parent_keeps_target() {
    let genesis = 1_800_000_000;
    assert_eq!(
        required_target(2, genesis + 300, genesis, &params()).unwrap(),
        target(1_000_000)
    );
}

#[test]
fn one_half_life_late_doubles_target() {
    let genesis = 1_800_000_000;
    assert_eq!(
        required_target(2, genesis + 300 + 21_600, genesis, &params()).unwrap(),
        target(2_000_000)
    );
}

#[test]
fn one_half_life_early_halves_target() {
    let genesis = 1_800_000_000;
    assert_eq!(
        required_target(2, genesis + 300 - 21_600, genesis, &params()).unwrap(),
        target(500_000)
    );
}

#[test]
fn half_half_life_fraction_matches_frozen_polynomial() {
    let genesis = 1_800_000_000;
    assert_eq!(
        required_target(2, genesis + 300 + 10_800, genesis, &params()).unwrap(),
        target(1_414_093)
    );
}
```

Also test a huge positive exponent clamps to `pow_limit` and huge negative exponent clamps to target 1.

- [ ] **Step 2: Run RED**

```bash
cargo +1.85.0 test -p oregon-consensus asert::tests --no-fail-fast
```

Expected: FAIL because `required_target` is absent.

- [ ] **Step 3: Implement the fixed-point algorithm exactly**

Use `i128` for signed exponent math and `BigUint` for target multiplication/shifting:

```rust
pub fn required_target(
    height: u64,
    parent_timestamp: u64,
    genesis_timestamp: u64,
    params: &ConsensusParams,
) -> Result<Target, ConsensusError> {
    if height == 0 {
        return Err(ConsensusError::InvalidHeight);
    }
    if height == 1 {
        return Ok(params.initial_target);
    }

    let time_delta = i128::from(parent_timestamp) - i128::from(genesis_timestamp);
    let height_delta = i128::from(height - 2);
    let ideal = 300i128
        .checked_mul(height_delta + 1)
        .ok_or(ConsensusError::ArithmeticOverflow)?;
    let numerator = (time_delta - ideal)
        .checked_mul(65_536)
        .ok_or(ConsensusError::ArithmeticOverflow)?;
    let exponent = numerator / 21_600; // Rust signed division truncates toward zero.

    let num_shifts = exponent >> 16; // arithmetic shift for i128
    let frac = exponent - num_shifts * 65_536;
    debug_assert!((0..65_536).contains(&frac));

    let frac2 = frac.checked_mul(frac).ok_or(ConsensusError::ArithmeticOverflow)?;
    let frac3 = frac2.checked_mul(frac).ok_or(ConsensusError::ArithmeticOverflow)?;
    let polynomial = 195_766_423_245_049i128
        .checked_mul(frac)
        .and_then(|v| v.checked_add(971_821_376i128 * frac2))
        .and_then(|v| v.checked_add(5_127i128 * frac3))
        .and_then(|v| v.checked_add(1i128 << 47))
        .ok_or(ConsensusError::ArithmeticOverflow)?;
    let factor = (polynomial >> 48) + 65_536;
    let factor = u64::try_from(factor).map_err(|_| ConsensusError::ArithmeticOverflow)?;

    if num_shifts >= 256 {
        return Ok(params.pow_limit);
    }
    if num_shifts <= -257 {
        return Target::from_biguint(&BigUint::from(1u8));
    }

    let mut candidate = params.initial_target.to_biguint() * BigUint::from(factor);
    if num_shifts < 0 {
        candidate >>= usize::try_from(-num_shifts).map_err(|_| ConsensusError::ArithmeticOverflow)?;
    } else {
        candidate <<= usize::try_from(num_shifts).map_err(|_| ConsensusError::ArithmeticOverflow)?;
    }
    candidate >>= 16usize;

    if candidate.is_zero() {
        return Target::from_biguint(&BigUint::from(1u8));
    }
    if candidate > params.pow_limit.to_biguint() {
        return Ok(params.pow_limit);
    }
    Target::from_biguint(&candidate)
}
```

Do not replace any of the frozen coefficients with floating-point `powf`, logarithms, or an approximate target conversion.

- [ ] **Step 4: Add property tests for schedule invariants**

```rust
proptest! {
    #[test]
    fn on_schedule_chain_keeps_initial_target(height in 2u64..100_000) {
        let genesis = 1_800_000_000u64;
        let parent_timestamp = genesis + 300 * (height - 1);
        prop_assert_eq!(
            required_target(height, parent_timestamp, genesis, &params()).unwrap(),
            target(1_000_000)
        );
    }
}
```

Also property-test that every result is non-zero and never exceeds `pow_limit` for bounded random heights/timestamps.

- [ ] **Step 5: Full gate, commit, reviewer math check**

```bash
cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
git add crates/oregon-consensus/src/asert.rs crates/oregon-consensus/src/lib.rs
git commit -m "feat: implement Oregon per-block ASERT"
```

Reviewer gate: check height-2 off-by-one explicitly, signed negative exponent behavior, `frac` range, `>=256`/`<=-257` short-circuit proofs, exact polynomial coefficients, and no float usage.

---

### Task 5: MTP, Chain Work, and Pre-PoW Header Context

**Files:**
- Modify: `crates/oregon-consensus/src/time.rs`
- Modify: `crates/oregon-consensus/src/work.rs`
- Modify: `crates/oregon-consensus/src/header.rs`
- Modify: `crates/oregon-consensus/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub fn median_time_past(window: &[u64]) -> Result<u64, ConsensusError>`
  - `pub struct ChainWork(BigUint)`
  - `pub fn block_work(target: Target) -> ChainWork`
  - `pub struct HeaderContext<'a> { pub height: u64, pub parent: &'a BlockHeader, pub genesis_timestamp: u64, pub mtp_window: &'a [u64] }`
  - `pub struct PrePowHeaderFacts { pub target: Target, pub work: ChainWork }`
  - `pub fn validate_header_pre_pow(header: &BlockHeader, context: &HeaderContext<'_>, params: &ConsensusParams) -> Result<PrePowHeaderFacts, ConsensusError>`

- [ ] **Step 1: Write failing MTP tests including early even windows**

```rust
#[test]
fn mtp_uses_upper_median_for_even_early_windows() {
    assert_eq!(median_time_past(&[100, 200]).unwrap(), 200);
}

#[test]
fn mtp_sorts_and_uses_index_floor_count_over_two() {
    let window = [90, 20, 80, 30, 70, 40, 60, 50, 10, 100, 110];
    assert_eq!(median_time_past(&window).unwrap(), 60);
}

#[test]
fn mtp_rejects_empty_or_more_than_eleven_values() {
    assert_eq!(median_time_past(&[]), Err(ConsensusError::InvalidMtpWindow));
    assert_eq!(median_time_past(&[0; 12]), Err(ConsensusError::InvalidMtpWindow));
}
```

- [ ] **Step 2: Write failing chain-work tests**

```rust
#[test]
fn easiest_full_width_target_has_one_unit_of_work() {
    let target = Target::from_le_bytes([0xff; 32]).unwrap();
    assert_eq!(block_work(target).to_biguint(), BigUint::from(1u8));
}

#[test]
fn target_one_has_two_to_the_255_work() {
    let target = Target::from_biguint(&BigUint::from(1u8)).unwrap();
    assert_eq!(block_work(target).to_biguint(), BigUint::from(1u8) << 255usize);
}

#[test]
fn harder_target_has_more_work() {
    let hard = Target::from_biguint(&100u32.into()).unwrap();
    let easy = Target::from_biguint(&200u32.into()).unwrap();
    assert!(block_work(hard) > block_work(easy));
}
```

- [ ] **Step 3: Write failing header-context tests**

Construct a parent and child header; assert:

```rust
assert_eq!(
    validate_header_pre_pow(&child_with_wrong_parent, &context, &params()),
    Err(ConsensusError::PreviousBlockMismatch)
);
assert_eq!(
    validate_header_pre_pow(&child_with_timestamp_equal_to_mtp, &context, &params()),
    Err(ConsensusError::TimestampNotAfterMtp)
);
assert_eq!(
    validate_header_pre_pow(&child_with_wrong_target, &context, &params()),
    Err(ConsensusError::UnexpectedTarget)
);
```

The valid case must return target/work facts but MUST NOT call RandomX or claim PoW validity.

- [ ] **Step 4: Run RED**

```bash
cargo +1.85.0 test -p oregon-consensus time::tests work::tests header::tests --no-fail-fast
```

Expected: missing functions/types fail.

- [ ] **Step 5: Implement MTP and chain work**

`time.rs`:

```rust
pub fn median_time_past(window: &[u64]) -> Result<u64, ConsensusError> {
    if window.is_empty() || window.len() > 11 {
        return Err(ConsensusError::InvalidMtpWindow);
    }
    let mut sorted = window.to_vec();
    sorted.sort_unstable();
    Ok(sorted[sorted.len() / 2])
}
```

`work.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChainWork(BigUint);

impl ChainWork {
    pub fn zero() -> Self { Self(BigUint::ZERO) }
    pub fn to_biguint(&self) -> BigUint { self.0.clone() }
    pub fn add_assign(&mut self, rhs: &Self) { self.0 += &rhs.0; }
}

pub fn block_work(target: Target) -> ChainWork {
    let numerator = BigUint::from(1u8) << 256usize;
    let denominator = target.to_biguint() + BigUint::from(1u8);
    ChainWork(numerator / denominator)
}
```

- [ ] **Step 6: Implement explicit pre-PoW header validation**

Core order:

```rust
pub fn validate_header_pre_pow(
    header: &BlockHeader,
    context: &HeaderContext<'_>,
    params: &ConsensusParams,
) -> Result<PrePowHeaderFacts, ConsensusError> {
    if context.height == 0 {
        return Err(ConsensusError::InvalidHeight);
    }
    if header.previous_block != context.parent.block_id() {
        return Err(ConsensusError::PreviousBlockMismatch);
    }
    let mtp = median_time_past(context.mtp_window)?;
    if header.timestamp <= mtp {
        return Err(ConsensusError::TimestampNotAfterMtp);
    }
    let expected = required_target(
        context.height,
        context.parent.timestamp,
        context.genesis_timestamp,
        params,
    )?;
    let actual = Target::from_le_bytes(header.difficulty_commitment)?;
    actual.validate_against(params.pow_limit)?;
    if actual != expected {
        return Err(ConsensusError::UnexpectedTarget);
    }
    Ok(PrePowHeaderFacts {
        target: actual,
        work: block_work(actual),
    })
}
```

- [ ] **Step 7: Full gate and commit**

```bash
cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
git add crates/oregon-consensus/src/time.rs crates/oregon-consensus/src/work.rs crates/oregon-consensus/src/header.rs crates/oregon-consensus/src/lib.rs
git commit -m "feat: validate Oregon header context before PoW"
```

Reviewer gate: verify header function name/documentation says pre-PoW, parent timestamp—not candidate timestamp—feeds ASERT, MTP uses candidate timestamp only as floor check, and work formula is exactly `floor(2^256/(target+1))`.

---

### Task 6: Non-Genesis Block Structural Validation

**Files:**
- Modify: `crates/oregon-consensus/src/block.rs`
- Modify: `crates/oregon-consensus/src/lib.rs`

**Interfaces:**
- Consumes: `Block`, `transaction_root`, `validate_coinbase`, precomputed `total_fees: Amount` from the future M3 state-transition layer.
- Produces:
  - `pub fn validate_non_genesis_block_structure(block: &Block, height: u64, total_fees: Amount, params: &ConsensusParams) -> Result<(), ConsensusError>`

This function does not validate signatures, UTXO existence, double spends, coinbase maturity, or RandomX. Those are explicit M2/M3 responsibilities.

- [ ] **Step 1: Write failing size/Merkle/coinbase uniqueness tests**

Create a valid small block helper with a valid coinbase and assert:

```rust
#[test]
fn valid_non_genesis_structure_passes() {
    assert_eq!(
        validate_non_genesis_block_structure(
            &valid_block(2),
            2,
            Amount::from_base_units(0).unwrap(),
            &params(),
        ),
        Ok(())
    );
}

#[test]
fn merkle_mutation_is_rejected() {
    let mut block = valid_block(2);
    block.header.transaction_root = Hash256::from_bytes([0x99; 32]);
    assert_eq!(
        validate_non_genesis_block_structure(
            &block, 2, Amount::from_base_units(0).unwrap(), &params()
        ),
        Err(ConsensusError::MerkleRootMismatch)
    );
}
```

Also construct a second coinbase-form transaction at index 1 and expect `MultipleCoinbase`.

- [ ] **Step 2: Add explicit canonical-byte limits**

Test by creating transactions with large witness/program bytes so `tx.encode().len() > 102_400`, and a block whose `block.encode().len() > 1_048_576`. Do not use foundation's 64 MiB decode limit as the expected economic limit.

Example assertion:

```rust
assert_eq!(
    validate_non_genesis_block_structure(&oversized, 2, zero_fees(), &params()),
    Err(ConsensusError::BlockTooLarge)
);
```

- [ ] **Step 3: Add null-outpoint-in-normal-transaction RED test**

A transaction after index 0 containing an input whose txid is all zero and index is `u32::MAX` must fail even if other fields differ from a valid coinbase.

- [ ] **Step 4: Run RED**

```bash
cargo +1.85.0 test -p oregon-consensus block::tests --no-fail-fast
```

Expected: FAIL before implementation.

- [ ] **Step 5: Implement cheap-to-expensive structural order**

```rust
pub fn validate_non_genesis_block_structure(
    block: &Block,
    height: u64,
    total_fees: Amount,
    params: &ConsensusParams,
) -> Result<(), ConsensusError> {
    if height == 0 {
        return Err(ConsensusError::InvalidHeight);
    }
    if block.encode().len() > MAX_BLOCK_BYTES {
        return Err(ConsensusError::BlockTooLarge);
    }
    if block.transactions.is_empty() {
        return Err(ConsensusError::EmptyNonGenesisBlock);
    }
    for (index, tx) in block.transactions.iter().enumerate() {
        if tx.encode().len() > MAX_TRANSACTION_BYTES {
            return Err(ConsensusError::TransactionTooLarge(index));
        }
    }
    if transaction_root(&block.transactions).map_err(|_| ConsensusError::MerkleRootMismatch)?
        != block.header.transaction_root
    {
        return Err(ConsensusError::MerkleRootMismatch);
    }

    validate_coinbase(&block.transactions[0], height, total_fees, params)?;

    for tx in &block.transactions[1..] {
        if is_coinbase_form(tx) {
            return Err(ConsensusError::MultipleCoinbase);
        }
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

- [ ] **Step 6: Full gate and commit**

```bash
cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
git add crates/oregon-consensus/src/block.rs crates/oregon-consensus/src/lib.rs
git commit -m "feat: enforce Oregon non-genesis block structure"
```

Reviewer gate: verify 1 MiB check happens before expensive body work, 100 KiB applies to coinbase too, Merkle recomputation uses the frozen v0 Oregon tree, and there is no accidental UTXO/PoW claim.

---

### Task 7: Golden Vectors, Mutation Sensitivity, and M1 Checkpoint

**Files:**
- Create: `tests/vectors/consensus-m1-v1.json`
- Create: `crates/oregon-consensus/tests/golden_vectors.rs`
- Create: `docs/checkpoints/OREGON_V1_M1_CONSENSUS_CORE.md`
- Modify: `docs/checkpoints/OREGON_V0_PROTOCOL_PROGRESS.md`

**Interfaces:**
- Freezes externally readable M1 outputs so later implementations/backends can prove parity.
- No new consensus algorithm is introduced in this task.

- [ ] **Step 1: Write the golden-vector consumer before the JSON fixture exists**

The fixture schema is exactly:

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

The Rust consumer must deserialize strings/numbers and call public `oregon-consensus` APIs; it must not duplicate production algorithms inside the test.

- [ ] **Step 2: Run RED with missing fixture**

```bash
cargo +1.85.0 test -p oregon-consensus --test golden_vectors
```

Expected: FAIL specifically because `tests/vectors/consensus-m1-v1.json` is absent.

- [ ] **Step 3: Add the exact fixture and make the consumer GREEN**

Add the JSON exactly with the schema/values above. Use a fixed synthetic `genesis_timestamp = 1_800_000_000`, `initial_target = 1_000_000`, `pow_limit = 10_000_000` in ASERT vector consumption.

Run:

```bash
cargo +1.85.0 test -p oregon-consensus --test golden_vectors
```

Expected: PASS.

- [ ] **Step 4: Perform mutation sensitivity on a throwaway branch**

Create a branch from the current M1 head:

```bash
git switch -c oregon-v1-m1-mutation-emission-asert
```

Mutation A: in `emission.rs`, change:

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

Expected: FAILURE at the 200,000/200,001 boundary and/or golden emission vectors.

Revert Mutation A, then Mutation B: change ASERT half-life constant used by implementation from `21_600` to `21_601` without changing tests.

Run:

```bash
cargo +1.85.0 test -p oregon-consensus asert::tests --test golden_vectors
```

Expected: FAILURE in at least one non-zero-exponent vector. Record exact failing test names/run identifiers in the checkpoint.

Delete/abandon the mutation branch after recording evidence; never merge it into M1.

- [ ] **Step 5: Run the exact final acceptance gate on the clean M1 branch**

```bash
git switch oregon-v1-m1-consensus-core
cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
```

Expected: all commands exit 0 with no skipped consensus tests caused by missing tools.

- [ ] **Step 6: Write the M1 checkpoint with exact evidence**

`docs/checkpoints/OREGON_V1_M1_CONSENSUS_CORE.md` must record:

```markdown
# Oregon v1 M1 Consensus Core Acceptance

- Base design spec: `docs/superpowers/specs/2026-09-02-oregon-pow-consensus-v1-design.md`
- Implementation plan: `docs/superpowers/plans/2026-09-03-oregon-m1-consensus-core.md`
- Branch: `oregon-v1-m1-consensus-core`
- Exact accepted commit: `<fill with the already-created final M1 commit SHA at execution time>`
- Exact CI run: `<fill with the already-completed successful run id at execution time>`
- Golden vectors: `tests/vectors/consensus-m1-v1.json`
- Mutation evidence: exact throwaway commit/run names from Step 4.

Accepted:
- full-width LE target model and bounds
- exact emission schedule
- height-1 founder/coinbase reward rules
- ASERT integer target calculation
- MTP-11 calculation
- per-block work formula
- pre-PoW header context checks
- non-genesis block size/Merkle/coinbase structural rules

Explicitly not accepted by M1:
- RandomX verification (M2)
- UTXO/signature/maturity validation (M3)
- persistent chain/reorg storage (M4)
- genesis/address/network tooling (M5)
- P2P/mempool/mining RPC
```

The `<fill ...>` markers above are not implementation placeholders: they are execution evidence fields whose values do not exist until the final commit and CI run exist. The executor MUST replace them with concrete SHA/run values before committing the checkpoint; a checkpoint containing angle-bracket markers is invalid.

- [ ] **Step 7: Append a historical pointer without rewriting v0 evidence**

At the end of `docs/checkpoints/OREGON_V0_PROTOCOL_PROGRESS.md`, append:

```markdown
## Next milestone

Oregon v1 M1 Consensus Core was developed separately from the frozen v0 foundation. See `docs/checkpoints/OREGON_V1_M1_CONSENSUS_CORE.md` for its acceptance evidence. The v0 foundation checkpoint remains unchanged and recoverable.
```

- [ ] **Step 8: Final docs-only commit and one more exact-head CI gate**

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

If CI runs remotely, wait for the exact final-head run and verify test/format/clippy all succeed before creating the recovery branch.

- [ ] **Step 9: Create the immutable recovery branch only after exact-head success**

```bash
git branch oregon-v1-checkpoint-m1-consensus-core-accepted-2026-09-03
git push origin oregon-v1-checkpoint-m1-consensus-core-accepted-2026-09-03
```

Verify the recovery branch SHA equals the exact successful M1 head SHA.

Reviewer gate: compare the final implementation against M1 scope in Sections 4, 6, 7, 8, 9, 10, 13, 18, and 20 of the master spec. Any missing target/subsidy/founder/ASERT/MTP/work/size/Merkle check is an Important or Critical issue. Any RandomX/UTXO/P2P implementation accidentally added to M1 is scope creep and must be removed or separately designed.

---

## Plan Self-Review Record

### Spec coverage

M1 requirements are mapped as follows:

- Target representation/bounds and chain profile parameters -> Task 1.
- Subsidy/halving/exact scheduled issuance -> Task 2.
- Coinbase structure, height commitment, founder allocation, reward ceiling -> Task 3.
- ASERT fixed-anchor full-target algorithm -> Task 4.
- MTP, chain-work formula, parent/target/time header context -> Task 5.
- 1 MiB block, 100 KiB transaction, Merkle root, unique/null coinbase structure -> Task 6.
- Cross-version artifact, mutation sensitivity, final verification/checkpoint -> Task 7.

The following master-spec areas are intentionally outside M1 and therefore have no M1 implementation task: RandomX (M2), signatures/UTXO/maturity (M3), storage/reorg persistence (M4), address/genesis/network profile (M5), P2P/sync (M6), mempool/mining/RPC (M7), public benchmark/mainnet freeze (M8).

### Type consistency

- Every difficulty API uses `Target`.
- Every chain parameter bundle uses `ConsensusParams`.
- Fee/subsidy values crossing primitive/consensus boundaries use `Amount`; scheduled totals use exact base-unit integer constants.
- ASERT returns `Target`; header validation consumes the same type and returns it in `PrePowHeaderFacts`.
- Chain work is never stored in `u64/u128`; it uses `ChainWork(BigUint)`.
- Block validation receives `total_fees: Amount` from the future M3 state transition; M1 does not invent fee validation.

### Acceptance principle

M1 is accepted only when the exact final head passes the full Rust workspace test/format/clippy gate and the deliberate emission and ASERT mutations are caught by the relevant tests. A green suite after either deliberate consensus mutation means the test coverage is insufficient and the milestone must remain unaccepted.
