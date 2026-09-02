# Oregon v0 Protocol Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first independent Oregon Rust workspace with consensus-safe primitive types, canonical encoding, deterministic identifiers, Merkle commitments, and protocol-v0 golden vectors.

**Architecture:** Oregon v0 starts as a small Rust workspace centered on `oregon-primitives`. Consensus bytes are encoded and decoded explicitly, amounts are integer-only, identifiers use domain-separated BLAKE3, and all hostile-input parsing is bounded and fallible. Bitcoin Core and archived Radium work are references only; no Bitcoin implementation code is copied into the workspace.

**Tech Stack:** Rust 1.85.0, Rust edition 2024, `blake3`, `thiserror`, `proptest`, `serde`/`serde_json` for test-vector tooling only.

**Spec:** `docs/superpowers/specs/2026-09-02-oregon-v0-protocol-design.md`

## Global Constraints

- Oregon is implemented independently from scratch in Rust; Bitcoin `.cpp`/`.h` implementation files are not copied or renamed into Oregon.
- Rust edition is `2024`; minimum supported Rust version is `1.85.0`.
- Consensus crates use no `unsafe` Rust in this milestone.
- Consensus binary encoding is explicit; do not use `bincode`, `postcard`, generic `serde` binary formats, or compiler-layout serialization.
- `1 OREG = 100,000,000` base units.
- Maximum supply is `1,000,000 OREG = 100,000,000,000,000` base units.
- Founder allocation is public protocol data: `50,000 OREG = 5,000,000,000,000` base units, exactly 5%, to be enforced in the later monetary-consensus milestone.
- Retained schedule parameters for later milestones are initial subsidy `2.375 OREG`, halving interval `200,000` blocks, and target block interval `300` seconds.
- Protocol-v0 object identifiers use 256-bit BLAKE3 with the domain strings defined in the spec.
- Consensus parsers must reject non-canonical encodings, trailing bytes, configured length-limit violations, and malformed/truncated input without panicking.

---

## File Structure

The implementation creates this focused workspace:

```text
Cargo.toml
rust-toolchain.toml
crates/
└── oregon-primitives/
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        ├── amount.rs
        ├── error.rs
        ├── hash.rs
        ├── encoding.rs
        ├── transaction.rs
        ├── merkle.rs
        └── block.rs
tests/
└── vectors/
    └── protocol-v0.json
```

Responsibilities:

- `amount.rs`: OREG base-unit representation, constants, checked arithmetic.
- `error.rs`: public typed decode/consensus-primitive errors.
- `hash.rs`: `Hash256`, hexadecimal display/parse, domain-separated BLAKE3 helper.
- `encoding.rs`: canonical little-endian primitives, Oregon varints, bounded decoder.
- `transaction.rs`: transaction containers, canonical bytes, decode, txid.
- `merkle.rs`: ordered transaction commitment tree with odd-node promotion.
- `block.rs`: block-header/block containers, canonical header bytes, block ID.
- `protocol-v0.json`: immutable cross-implementation fixtures generated from reviewed Oregon code.

---

### Task 1: Workspace and Amount Safety

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `crates/oregon-primitives/Cargo.toml`
- Create: `crates/oregon-primitives/src/lib.rs`
- Create: `crates/oregon-primitives/src/error.rs`
- Create: `crates/oregon-primitives/src/amount.rs`

**Interfaces:**
- Produces: `pub struct Amount(u64)`
- Produces: `pub const BASE_UNITS_PER_OREG: u64 = 100_000_000`
- Produces: `pub const MAX_SUPPLY_BASE_UNITS: u64 = 100_000_000_000_000`
- Produces: `pub const FOUNDER_ALLOCATION_BASE_UNITS: u64 = 5_000_000_000_000`
- Produces: `Amount::from_base_units(u64) -> Result<Amount, PrimitiveError>`
- Produces: `Amount::base_units(self) -> u64`
- Produces: `Amount::checked_add(self, Amount) -> Result<Amount, PrimitiveError>`
- Produces: `Amount::checked_sub(self, Amount) -> Result<Amount, PrimitiveError>`

- [ ] **Step 1: Write failing amount-boundary tests**

Create `amount.rs` initially with only the test module and unresolved references:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monetary_constants_are_exact() {
        assert_eq!(BASE_UNITS_PER_OREG, 100_000_000);
        assert_eq!(MAX_SUPPLY_BASE_UNITS, 100_000_000_000_000);
        assert_eq!(FOUNDER_ALLOCATION_BASE_UNITS, 5_000_000_000_000);
        assert_eq!(FOUNDER_ALLOCATION_BASE_UNITS * 20, MAX_SUPPLY_BASE_UNITS);
    }

    #[test]
    fn amount_rejects_values_above_supply_envelope() {
        assert!(Amount::from_base_units(MAX_SUPPLY_BASE_UNITS).is_ok());
        assert!(Amount::from_base_units(MAX_SUPPLY_BASE_UNITS + 1).is_err());
    }

    #[test]
    fn amount_checked_add_never_wraps_or_exceeds_supply() {
        let max = Amount::from_base_units(MAX_SUPPLY_BASE_UNITS).unwrap();
        let one = Amount::from_base_units(1).unwrap();
        assert!(max.checked_add(one).is_err());
    }

    #[test]
    fn amount_checked_sub_rejects_underflow() {
        let zero = Amount::from_base_units(0).unwrap();
        let one = Amount::from_base_units(1).unwrap();
        assert!(zero.checked_sub(one).is_err());
    }
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test -p oregon-primitives amount::tests -- --nocapture
```

Expected: compilation failure because `Amount` and constants are not implemented yet.

- [ ] **Step 3: Implement workspace, typed error, and Amount**

`crates/oregon-primitives/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PrimitiveError {
    #[error("amount arithmetic overflow")]
    AmountOverflow,
    #[error("amount arithmetic underflow")]
    AmountUnderflow,
    #[error("amount exceeds Oregon maximum supply")]
    AmountAboveMaximum,
    #[error("unexpected end of input")]
    UnexpectedEof,
    #[error("non-canonical varint")]
    NonCanonicalVarInt,
    #[error("decoded length exceeds configured limit")]
    LengthLimitExceeded,
    #[error("invalid protocol version {0}")]
    InvalidVersion(u16),
    #[error("trailing bytes after complete consensus object")]
    TrailingBytes,
    #[error("invalid fixed-size hash length: expected 32, got {0}")]
    InvalidHashLength(usize),
    #[error("invalid lowercase hexadecimal hash")]
    InvalidHashHex,
    #[error("block transaction list must not be empty")]
    EmptyBlockTransactions,
}
```

`crates/oregon-primitives/src/amount.rs`:

```rust
use crate::PrimitiveError;

pub const BASE_UNITS_PER_OREG: u64 = 100_000_000;
pub const MAX_SUPPLY_BASE_UNITS: u64 = 1_000_000 * BASE_UNITS_PER_OREG;
pub const FOUNDER_ALLOCATION_BASE_UNITS: u64 = 50_000 * BASE_UNITS_PER_OREG;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Amount(u64);

impl Amount {
    pub fn from_base_units(value: u64) -> Result<Self, PrimitiveError> {
        if value > MAX_SUPPLY_BASE_UNITS {
            return Err(PrimitiveError::AmountAboveMaximum);
        }
        Ok(Self(value))
    }

    pub const fn base_units(self) -> u64 {
        self.0
    }

    pub fn checked_add(self, rhs: Self) -> Result<Self, PrimitiveError> {
        let value = self.0.checked_add(rhs.0).ok_or(PrimitiveError::AmountOverflow)?;
        Self::from_base_units(value)
    }

    pub fn checked_sub(self, rhs: Self) -> Result<Self, PrimitiveError> {
        let value = self.0.checked_sub(rhs.0).ok_or(PrimitiveError::AmountUnderflow)?;
        Self::from_base_units(value)
    }
}
```

Export modules from `lib.rs` and configure the workspace with `resolver = "3"`, edition 2024, and the exact dependencies in the spec.

- [ ] **Step 4: Run tests and verify GREEN**

Run:

```bash
cargo test -p oregon-primitives amount::tests -- --nocapture
cargo fmt --all -- --check
cargo clippy -p oregon-primitives --all-targets -- -D warnings
```

Expected: all amount tests pass; fmt and clippy exit 0.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml rust-toolchain.toml crates/oregon-primitives
git commit -m "feat: establish Oregon primitive amount model"
```

---

### Task 2: Hash256 and Domain-Separated Object Hashing

**Files:**
- Create: `crates/oregon-primitives/src/hash.rs`
- Modify: `crates/oregon-primitives/src/lib.rs`

**Interfaces:**
- Consumes: `PrimitiveError`
- Produces: `pub struct Hash256([u8; 32])`
- Produces: `Hash256::from_bytes([u8; 32]) -> Hash256`
- Produces: `Hash256::as_bytes(&self) -> &[u8; 32]`
- Produces: `Hash256::from_slice(&[u8]) -> Result<Hash256, PrimitiveError>`
- Produces: lowercase hexadecimal `Display` and `FromStr`
- Produces: `pub(crate) fn domain_hash(domain: &[u8], payload: &[u8]) -> Hash256`

- [ ] **Step 1: Write failing tests for fixed length, hex round-trip, and domain separation**

```rust
#[test]
fn hash_requires_exactly_32_bytes() {
    assert!(Hash256::from_slice(&[0u8; 31]).is_err());
    assert!(Hash256::from_slice(&[0u8; 32]).is_ok());
    assert!(Hash256::from_slice(&[0u8; 33]).is_err());
}

#[test]
fn hash_hex_is_lowercase_and_round_trips() {
    let hash = Hash256::from_bytes([0xab; 32]);
    let text = hash.to_string();
    assert_eq!(text.len(), 64);
    assert!(text.bytes().all(|b| !b.is_ascii_uppercase()));
    assert_eq!(text.parse::<Hash256>().unwrap(), hash);
}

#[test]
fn domains_change_hash_identity() {
    let payload = b"same payload";
    assert_ne!(
        domain_hash(b"OREGON/TX/V0\0", payload),
        domain_hash(b"OREGON/BLOCK/V0\0", payload)
    );
}
```

- [ ] **Step 2: Run test and verify RED**

Run:

```bash
cargo test -p oregon-primitives hash::tests -- --nocapture
```

Expected: compilation failure because `Hash256` and `domain_hash` do not exist.

- [ ] **Step 3: Implement `Hash256` and BLAKE3 domain hashing**

Use `blake3::Hasher` and feed the domain bytes first, then the canonical payload. Do not concatenate through temporary platform-layout structures.

- [ ] **Step 4: Run focused and full crate checks**

```bash
cargo test -p oregon-primitives hash::tests -- --nocapture
cargo test -p oregon-primitives
cargo fmt --all -- --check
cargo clippy -p oregon-primitives --all-targets -- -D warnings
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/oregon-primitives/src/hash.rs crates/oregon-primitives/src/lib.rs
git commit -m "feat: add Oregon domain separated hash identifiers"
```

---

### Task 3: Canonical Integer Encoding and Bounded Decoder

**Files:**
- Create: `crates/oregon-primitives/src/encoding.rs`
- Modify: `crates/oregon-primitives/src/lib.rs`

**Interfaces:**
- Consumes: `PrimitiveError`
- Produces: `pub struct DecodeLimits`
- Produces: `impl Default for DecodeLimits`
- Produces: `pub struct Decoder<'a>`
- Produces: little-endian `read_u16`, `read_u32`, `read_u64`
- Produces: `read_varint() -> Result<u64, PrimitiveError>`
- Produces: `read_len(max: usize) -> Result<usize, PrimitiveError>`
- Produces: `read_bytes(len: usize) -> Result<&'a [u8], PrimitiveError>`
- Produces: `finish() -> Result<(), PrimitiveError>`
- Produces: `write_varint(u64, &mut Vec<u8>)`

- [ ] **Step 1: Write failing varint boundary and rejection tests**

Test exact encodings:

```text
0xfc -> fc
0xfd -> fd fd 00
0xffff -> fd ff ff
0x1_0000 -> fe 00 00 01 00
0xffff_ffff -> fe ff ff ff ff
0x1_0000_0000 -> ff 00 00 00 00 01 00 00 00
```

Also test rejection of:

```text
fd fc 00
fe ff ff 00 00
ff ff ff ff ff 00 00 00 00
```

because each uses a wider form than necessary.

- [ ] **Step 2: Run test and verify RED**

```bash
cargo test -p oregon-primitives encoding::tests -- --nocapture
```

Expected: compilation failure for missing decoder/varint functions.

- [ ] **Step 3: Implement explicit little-endian reads/writes and canonical varints**

`DecodeLimits::default()` must use the spec values:

```rust
DecodeLimits {
    max_transaction_inputs: 65_535,
    max_transaction_outputs: 65_535,
    max_witness_items_per_input: 1_024,
    max_witness_item_bytes: 1_048_576,
    max_locking_program_bytes: 65_536,
    max_block_transactions: 1_000_000,
    max_object_bytes: 67_108_864,
}
```

Before converting any decoded `u64` length to `usize`, use `usize::try_from` and map failure to `LengthLimitExceeded`.

- [ ] **Step 4: Add truncation and trailing-byte tests**

Test every fixed-width read with input one byte too short. Test `finish()` returns `TrailingBytes` when one byte remains.

- [ ] **Step 5: Run full checks**

```bash
cargo test -p oregon-primitives
cargo fmt --all -- --check
cargo clippy -p oregon-primitives --all-targets -- -D warnings
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/oregon-primitives/src/encoding.rs crates/oregon-primitives/src/lib.rs
git commit -m "feat: add canonical Oregon consensus encoding"
```

---

### Task 4: Transaction Primitive, Canonical Bytes, and TxID

**Files:**
- Create: `crates/oregon-primitives/src/transaction.rs`
- Modify: `crates/oregon-primitives/src/lib.rs`
- Modify: `crates/oregon-primitives/src/encoding.rs`

**Interfaces:**
- Consumes: `Amount`, `Hash256`, `Decoder`, `DecodeLimits`, `domain_hash`
- Produces: `pub struct OutPoint { pub txid: Hash256, pub index: u32 }`
- Produces: `pub struct TxInput { pub previous_txid: Hash256, pub previous_output_index: u32, pub sequence: u32, pub witness: Vec<Vec<u8>> }`
- Produces: `pub struct TxOutput { pub value: Amount, pub locking_program: Vec<u8> }`
- Produces: `pub struct Transaction { pub version: u16, pub inputs: Vec<TxInput>, pub outputs: Vec<TxOutput>, pub lock_time: u64 }`
- Produces: `Transaction::encode(&self) -> Vec<u8>`
- Produces: `Transaction::decode(bytes: &[u8], limits: &DecodeLimits) -> Result<Transaction, PrimitiveError>`
- Produces: `Transaction::txid(&self) -> Hash256`

- [ ] **Step 1: Write failing minimum-version and round-trip tests**

Minimum valid test transaction:

```rust
let tx = Transaction {
    version: 1,
    inputs: vec![],
    outputs: vec![],
    lock_time: 0,
};
```

Assert version 0 fails decoding as `InvalidVersion(0)` and version 1 round-trips exactly.

- [ ] **Step 2: Write failing witness-commits-to-txid test**

Construct two otherwise equal transactions that differ only by one witness byte and assert their txids differ.

- [ ] **Step 3: Run tests and verify RED**

```bash
cargo test -p oregon-primitives transaction::tests -- --nocapture
```

Expected: compilation failure because transaction primitives do not exist.

- [ ] **Step 4: Implement transaction encode/decode exactly in spec order**

Encoding order:

```text
version:u16
input_count:varint
  previous_txid:32 bytes
  previous_output_index:u32
  sequence:u32
  witness_item_count:varint
    witness_item_len:varint
    witness_item:bytes
output_count:varint
  value:u64 base units
  locking_program_len:varint
  locking_program:bytes
lock_time:u64
```

Decoder must enforce every relevant `DecodeLimits` field before allocating vectors or byte buffers.

- [ ] **Step 5: Add hostile-length and trailing-byte tests**

Test an encoded input count above `max_transaction_inputs`, a witness item above `max_witness_item_bytes`, a locking program above `max_locking_program_bytes`, truncation at several boundaries, and one trailing byte after an otherwise valid transaction.

- [ ] **Step 6: Add property test for canonical round-trip**

Generate bounded valid transactions with `proptest`; for every generated value:

```rust
let encoded = tx.encode();
let decoded = Transaction::decode(&encoded, &limits).unwrap();
prop_assert_eq!(decoded.encode(), encoded);
prop_assert_eq!(decoded, tx);
```

Keep generated collections deliberately small so CI runtime is predictable.

- [ ] **Step 7: Run full checks and commit**

```bash
cargo test -p oregon-primitives
cargo fmt --all -- --check
cargo clippy -p oregon-primitives --all-targets -- -D warnings
git add crates/oregon-primitives/src
git commit -m "feat: define Oregon v0 transaction format"
```

---

### Task 5: Merkle Commitment, Block Header, and Block ID

**Files:**
- Create: `crates/oregon-primitives/src/merkle.rs`
- Create: `crates/oregon-primitives/src/block.rs`
- Modify: `crates/oregon-primitives/src/lib.rs`

**Interfaces:**
- Consumes: `Hash256`, `Transaction`, `Decoder`, `DecodeLimits`, `domain_hash`
- Produces: `pub fn transaction_root(transactions: &[Transaction]) -> Result<Hash256, PrimitiveError>`
- Produces: `pub struct BlockHeader { pub version: u16, pub previous_block: Hash256, pub transaction_root: Hash256, pub timestamp: u64, pub difficulty_commitment: [u8; 32], pub nonce: u64 }`
- Produces: `pub struct Block { pub header: BlockHeader, pub transactions: Vec<Transaction> }`
- Produces: `BlockHeader::encode(&self) -> Vec<u8>`
- Produces: `BlockHeader::decode(bytes: &[u8]) -> Result<BlockHeader, PrimitiveError>`
- Produces: `BlockHeader::block_id(&self) -> Hash256`
- Produces: `Block::encode(&self) -> Vec<u8>`
- Produces: `Block::decode(bytes: &[u8], limits: &DecodeLimits) -> Result<Block, PrimitiveError>`

- [ ] **Step 1: Write failing Merkle tests for 1, 2, and 3 transactions**

The 3-transaction test must manually compute:

```text
L0 = leaf(tx0)
L1 = leaf(tx1)
L2 = leaf(tx2)
P0 = node(L0, L1)
root = node(P0, L2)
```

and assert Oregon does not duplicate `L2`.

- [ ] **Step 2: Run Merkle tests and verify RED**

```bash
cargo test -p oregon-primitives merkle::tests -- --nocapture
```

Expected: compilation failure because Merkle code does not exist.

- [ ] **Step 3: Implement domain-separated Merkle tree**

Use exactly:

```text
leaf = BLAKE3("OREGON/MERKLE-LEAF/V0\0" || txid)
node = BLAKE3("OREGON/MERKLE/V0\0" || left || right)
```

For odd levels, promote the final node unchanged.

- [ ] **Step 4: Write failing block-header and block tests**

Assert:

- header encode/decode round-trip;
- changing nonce changes block ID;
- changing only transactions without updating `transaction_root` does not change header ID, demonstrating that header identity is structurally separate from block-body validation;
- empty transaction list is rejected;
- block transaction count respects `max_block_transactions`.

- [ ] **Step 5: Implement header and block encoding**

Header encoding is fixed-width and exactly:

```text
version:u16
previous_block:[u8;32]
transaction_root:[u8;32]
timestamp:u64
difficulty_commitment:[u8;32]
nonce:u64
```

Block encoding is header bytes followed by transaction-count varint and canonical transaction encodings.

- [ ] **Step 6: Run full checks and commit**

```bash
cargo test -p oregon-primitives
cargo fmt --all -- --check
cargo clippy -p oregon-primitives --all-targets -- -D warnings
git add crates/oregon-primitives/src
git commit -m "feat: add Oregon block and Merkle primitives"
```

---

### Task 6: Golden Protocol-v0 Vectors

**Files:**
- Modify: `crates/oregon-primitives/Cargo.toml`
- Add tests in the appropriate primitive modules or create: `crates/oregon-primitives/tests/golden_vectors.rs`
- Create: `tests/vectors/protocol-v0.json`

**Interfaces:**
- Consumes: all Task 1-5 public primitive interfaces
- Produces: immutable JSON fixture file with exact canonical bytes and hashes

- [ ] **Step 1: Add test-only JSON fixture schema**

Use `serde`/`serde_json` only in integration-test code. Define fields that store bytes as lowercase hex strings and amounts as integer base-unit values, never JSON floating point.

The checked-in JSON must contain at least:

1. canonical varint boundaries;
2. explicit non-minimal varint rejection cases;
3. minimum valid version-1 transaction;
4. multi-input/output/witness transaction;
5. exact canonical transaction bytes and txid;
6. one-transaction Merkle root;
7. two-transaction Merkle root;
8. three-transaction odd-promotion Merkle root;
9. exact block-header bytes and block ID;
10. amount maximum and above-maximum rejection values.

- [ ] **Step 2: Write the golden-vector consumer test before checking in fixtures**

The integration test must read `../../tests/vectors/protocol-v0.json`, reconstruct the objects, and compare every expected byte string/hash against the current implementation.

- [ ] **Step 3: Run test and verify RED because fixture file is absent**

```bash
cargo test -p oregon-primitives --test golden_vectors -- --nocapture
```

Expected: failure opening the missing fixture file.

- [ ] **Step 4: Generate vectors once with a reviewable helper and check in only deterministic output**

Create a temporary example/tool if helpful, run it once, inspect the JSON, then either remove the generator or keep it only if it has a clear reproducibility purpose. The golden fixture itself is the protocol artifact and must not contain timestamps, random values, filesystem paths, or environment-dependent data.

- [ ] **Step 5: Run golden-vector test and verify GREEN**

```bash
cargo test -p oregon-primitives --test golden_vectors -- --nocapture
```

Expected: all fixture assertions pass.

- [ ] **Step 6: Mutation-sensitivity check**

Temporarily alter one consensus-critical rule locally—for example allow `0xfd fc 00` as canonical or duplicate the final odd Merkle node—and confirm at least one existing test fails. Revert the deliberate mutation immediately after recording the result.

- [ ] **Step 7: Run full acceptance gate**

```bash
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: exit code 0 for all three commands.

- [ ] **Step 8: Commit**

```bash
git add crates/oregon-primitives tests/vectors/protocol-v0.json
git commit -m "test: freeze Oregon protocol v0 golden vectors"
```

---

### Task 7: Foundation Acceptance Record

**Files:**
- Create: `docs/checkpoints/OREGON_V0_PROTOCOL_FOUNDATION.md`
- Modify: `README.md` only if needed to point at the independent Rust protocol branch and design without claiming a runnable mainnet.

**Interfaces:**
- Consumes: verified outputs and commit SHAs from Tasks 1-6
- Produces: a concise checkpoint stating what is frozen and what remains intentionally out of scope

- [ ] **Step 1: Write checkpoint with exact verified scope**

Record:

- branch name `oregon-v0-protocol`;
- design spec path;
- implementation-plan path;
- Rust/MSRV values;
- monetary constants;
- canonical encoding and hash-domain identifiers;
- golden vector path;
- exact acceptance commands used;
- explicit non-goals: no runnable P2P node, no production genesis, no final PoW/difficulty, no wallet, no mining RPC.

Do not claim future components exist.

- [ ] **Step 2: Run final fresh verification**

```bash
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all exit 0. Capture the command outcomes in the checkpoint.

- [ ] **Step 3: Review source tree for independence rule**

Run a repository search over the new Rust workspace for accidental Bitcoin implementation imports, copied `.cpp`/`.h` files, `bitcoin::` crate dependencies, `bincode`, `postcard`, and `unsafe`. Any occurrence must either be absent or an explanatory documentation reference rather than implementation coupling.

- [ ] **Step 4: Commit checkpoint**

```bash
git add docs/checkpoints/OREGON_V0_PROTOCOL_FOUNDATION.md README.md
git commit -m "docs: record Oregon v0 protocol foundation checkpoint"
```

---

## Plan Self-Review

### Spec coverage

- Independent Rust workspace: Task 1.
- Integer-only monetary envelope and founder-allocation constants: Task 1.
- Typed errors: Tasks 1 and 3-5.
- Domain-separated BLAKE3 IDs: Task 2, consumed in Tasks 4-5.
- Explicit canonical varints and bounded parsing: Task 3.
- Transaction canonical format and witness-committed txid: Task 4.
- Non-duplicating odd-node Merkle rule: Task 5.
- Fixed Oregon block-header primitive with opaque 32-byte difficulty commitment: Task 5.
- Golden vectors: Task 6.
- Round-trip/property/hostile-input testing: Tasks 3-6.
- Mutation-sensitivity rule: Task 6.
- No false claim of a runnable blockchain: Task 7.

### Placeholder scan

This plan intentionally contains no `TBD`, `TODO`, “implement later”, or unspecified test instruction. Deferred protocol areas are explicit non-goals from the approved spec and not placeholders inside this implementation package.

### Type consistency

The public types used across tasks are fixed as `Amount`, `PrimitiveError`, `Hash256`, `DecodeLimits`, `Decoder`, `OutPoint`, `TxInput`, `TxOutput`, `Transaction`, `BlockHeader`, and `Block`. Later tasks consume the exact names defined by earlier tasks.
