# Oregon v0 Protocol Foundation Design

## Status

Approved direction: Oregon is an independent blockchain protocol implemented from scratch in Rust. Bitcoin Core and the archived Radium M0A work are research, behavior, security, and test references only; Oregon source code must not be produced by copying Bitcoin implementation files or renaming a fork.

This document covers only the first implementation sub-project: protocol primitives, canonical binary encoding, deterministic identifiers, and monetary constants. P2P, persistent storage, mempool, wallet, mining RPC, proof-of-work selection, difficulty adjustment, genesis construction, and production networking are separate later designs.

## Goals

1. Establish Oregon-owned consensus data types that do not depend on Bitcoin source code.
2. Make every consensus object have exactly one canonical byte representation.
3. Make transaction and block identifiers deterministic across platforms and implementations.
4. Represent OREG amounts without floating point.
5. Encode the already-approved Oregon monetary envelope without yet implementing block subsidy validation.
6. Produce golden test vectors that future Rust, hardware-wallet, explorer, miner, and independent-node implementations can share.

## Non-goals

- No Bitcoin wire compatibility.
- No Bitcoin RPC compatibility.
- No Bitcoin address compatibility.
- No P2P networking in this milestone.
- No signature script or VM design in this milestone.
- No final PoW algorithm or difficulty algorithm in this milestone.
- No production genesis block in this milestone.
- No wallet or private-key handling in this milestone.

## Independence rule

Oregon may study public protocol specifications, academic papers, security failures, test cases, and externally observable behavior from Bitcoin, Kaspa, Monero, Ergo, Alephium, and other systems. Oregon implementation code is written independently around Oregon's own interfaces and serialized formats.

The repository must not import Bitcoin `.cpp`/`.h` source files, preserve Bitcoin class layouts merely for compatibility, or use Bitcoin serialization as the default because it is familiar. When a design choice matches another protocol, the Oregon documentation should state the engineering reason for that choice.

## Language and workspace

- Implementation language: Rust.
- Rust edition: 2024.
- Minimum supported Rust version for the first workspace: 1.85.0.
- Consensus crates must compile without unsafe Rust unless a later reviewed design explicitly grants a narrow exception.
- Consensus encoding must not use `bincode`, `postcard`, generic `serde` binary formats, or compiler-layout serialization. Consensus bytes are written and parsed explicitly.

Initial workspace shape:

```text
oregon/
├── Cargo.toml
├── rust-toolchain.toml
├── crates/
│   └── oregon-primitives/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── amount.rs
│           ├── hash.rs
│           ├── encoding.rs
│           ├── transaction.rs
│           └── block.rs
└── tests/
    └── vectors/
        └── protocol-v0.json
```

The root workspace will grow by new focused crates in later milestones rather than turning `oregon-primitives` into a node monolith.

## Monetary representation

The native asset is OREG.

- `1 OREG = 100,000,000` base units.
- Base-unit values are unsigned 64-bit integers.
- Floating-point values are forbidden in consensus amount calculations.
- Maximum supply envelope: `1,000,000 OREG = 100,000,000,000,000` base units.
- Founder allocation: `50,000 OREG = 5,000,000,000,000` base units, exactly 5% of the maximum envelope.
- Founder allocation is a one-time public consensus allocation at height 1 in the later monetary-consensus milestone. It is not a recurring tax.
- Mining envelope: at most the remaining 95% of the maximum supply.
- Existing approved schedule parameters retained for later implementation: initial subsidy `2.375 OREG`, halving interval `200,000` blocks, target block interval `300` seconds.
- Under integer halving at 1e-8 precision, the previously approved schedule emits `949,999.97 OREG` through mining, leaving `0.03 OREG` permanently unreachable below the 1,000,000 OREG cap. This slack is intentional unless a future consensus design explicitly changes the schedule before mainnet.

`Amount` is a newtype around `u64`; arithmetic that can overflow or exceed `MAX_SUPPLY_BASE_UNITS` returns an error. No implicit saturation or wrapping is allowed.

## Hash and identifier model

Protocol v0 uses 256-bit BLAKE3 outputs for object identifiers, with explicit domain separation so the same bytes cannot silently mean two different consensus objects.

Domains:

- Transaction ID preimage prefix: ASCII `OREGON/TX/V0\0`
- Block-header ID preimage prefix: ASCII `OREGON/BLOCK/V0\0`
- Merkle internal-node prefix: ASCII `OREGON/MERKLE/V0\0`
- Merkle leaf prefix: ASCII `OREGON/MERKLE-LEAF/V0\0`

A transaction ID is `BLAKE3(domain || canonical_transaction_bytes)`.

A block ID is `BLAKE3(domain || canonical_block_header_bytes)`.

PoW may later use the block-header bytes and a different reviewed work function. Therefore object identity and proof-of-work are separate interfaces from the first milestone.

All hashes are stored internally as exactly 32 bytes. Human-readable formatting uses lowercase hexadecimal and is never part of consensus encoding.

## Integer encoding

Consensus integers are little-endian.

Fixed-width fields use their exact widths (`u16`, `u32`, `u64`). Collection lengths and byte-string lengths use Oregon canonical varints:

- one to nine bytes;
- first byte values `0x00..=0xfc` encode themselves;
- `0xfd` is followed by a little-endian `u16` and is valid only for values `>= 0xfd`;
- `0xfe` is followed by a little-endian `u32` and is valid only for values `> u16::MAX`;
- `0xff` is followed by a little-endian `u64` and is valid only for values `> u32::MAX`.

Non-minimal encodings are consensus-invalid. Parsers must reject truncation, integer overflow, allocation lengths above configured parsing limits, and trailing bytes when decoding a complete consensus object.

## Transaction primitive

Protocol-v0 transaction fields are:

```text
Transaction {
    version: u16,
    inputs: Vec<TxInput>,
    outputs: Vec<TxOutput>,
    lock_time: u64,
}

TxInput {
    previous_txid: Hash256,
    previous_output_index: u32,
    sequence: u32,
    witness: Vec<Vec<u8>>,
}

TxOutput {
    value: Amount,
    locking_program: Vec<u8>,
}
```

Version `0` is reserved. The first valid normal transaction version is `1`.

`lock_time` semantics, witness validation, signature rules, coinbase structure, and locking-program execution are intentionally not defined here. This milestone defines stable containers and bytes without prematurely choosing the scripting system.

Transaction canonical encoding order is exactly the field order above. Vector counts precede vector contents. Every witness item and locking program is length-prefixed with canonical varints.

The transaction identifier commits to witness bytes. Oregon v0 therefore has one transaction identity rather than separate transaction/witness identifiers. Changing witness bytes changes the transaction ID. If future research shows a strong reason to separate them, that requires a new protocol-version design before mainnet.

## Outpoint primitive

Later UTXO code will refer to outputs using:

```text
OutPoint {
    txid: Hash256,
    index: u32,
}
```

`TxInput` exposes its previous output as an `OutPoint` value but serializes the two fields directly in the order specified above.

## Block primitive

Protocol-v0 block header fields are:

```text
BlockHeader {
    version: u16,
    previous_block: Hash256,
    transaction_root: Hash256,
    timestamp: u64,
    difficulty_commitment: [u8; 32],
    nonce: u64,
}

Block {
    header: BlockHeader,
    transactions: Vec<Transaction>,
}
```

`difficulty_commitment` is deliberately opaque at this stage. The future PoW/difficulty design owns its interpretation while the header format already reserves a fixed 32-byte consensus commitment. This avoids binding the foundation milestone to Bitcoin `nBits` or another chain's difficulty representation.

Timestamp is an unsigned Unix-seconds value. Timestamp validity rules are deferred to chain consensus.

The block ID commits only to the canonical block header. Transaction contents are committed through `transaction_root`.

## Transaction commitment tree

Blocks commit to ordered transaction IDs with a binary Merkle tree using the domain-separated BLAKE3 rules above.

- Leaf hash: `BLAKE3(OREGON/MERKLE-LEAF/V0\0 || txid)`.
- Internal hash: `BLAKE3(OREGON/MERKLE/V0\0 || left || right)`.
- For an odd level, the last node is promoted unchanged to the next level; it is not duplicated.
- An empty transaction list is invalid for a block, so no empty-root constant is defined.

The non-duplication rule is intentional and must have explicit golden vectors because it differs from Bitcoin's historical duplicate-last convention and avoids ambiguity related to duplicated terminal nodes.

## Parsing limits

Canonical encoding is independent from defensive parsing limits, but the first implementation exposes a `DecodeLimits` structure so callers must choose bounded memory behavior.

Initial default limits for tests and future node use:

- maximum transaction inputs: 65,535;
- maximum transaction outputs: 65,535;
- maximum witness items per input: 1,024;
- maximum individual witness item: 1 MiB;
- maximum locking program: 64 KiB;
- maximum transactions per decoded block: 1,000,000;
- maximum complete consensus object bytes accepted by convenience decoders: 64 MiB.

These are decoder safety limits, not final block-consensus limits. Final block weight/size policy is a later design.

## Error model

`oregon-primitives` returns typed errors. At minimum the public error categories distinguish:

- unexpected end of input;
- non-canonical varint;
- length limit exceeded;
- invalid version;
- amount overflow;
- amount above maximum supply envelope;
- trailing bytes;
- invalid fixed-size hash length.

Consensus parsing must not panic on hostile byte input.

## Golden vectors

`tests/vectors/protocol-v0.json` will contain human-readable fixtures for at least:

1. zero and boundary canonical varints;
2. rejection examples for non-minimal varints;
3. minimum valid version-1 transaction;
4. transaction with multiple inputs, outputs, and witness items;
5. exact transaction canonical bytes and expected txid;
6. one-transaction block root;
7. two-transaction root;
8. three-transaction root proving odd-node promotion;
9. exact block-header bytes and expected block ID;
10. maximum-supply amount boundaries and overflow rejection cases.

Vectors are generated once by reviewed Oregon code and then checked in as immutable protocol fixtures. A later independent implementation should be able to reproduce them without linking the Rust crate.

## Testing requirements

Foundation acceptance requires:

- unit tests for every amount boundary;
- round-trip encode/decode tests;
- canonical-varint rejection tests;
- deterministic golden-vector tests;
- property tests asserting that successful decode followed by encode produces exactly the consumed canonical bytes;
- hostile-input tests proving parsers return errors rather than panic;
- mutation tests around consensus-critical checks where practical.

Project-wide rule inherited from the prior research work: if a consensus-critical validation check is deliberately removed and the relevant test suite remains green, the test coverage is insufficient and the milestone is not accepted.

## Dependency policy

The first primitives crate should keep dependencies intentionally small:

- `blake3` for 256-bit hashing;
- `thiserror` for typed errors;
- `proptest` as a dev dependency for property tests;
- `serde`/`serde_json` only in test/vector tooling, not as the consensus binary codec.

Additional dependencies require a concrete need rather than convenience.

## Security properties to preserve

1. No floating-point consensus values.
2. No platform-dependent struct serialization.
3. No ambiguous/non-minimal length encoding.
4. No unbounded allocations directly from attacker-controlled lengths.
5. Object-type hash domain separation.
6. Merkle construction with an explicit odd-node rule.
7. No private keys, founder secrets, seeds, or production genesis secrets in the repository.
8. Founder allocation amount is public protocol data; founder private-key material is outside source control.

## Forward compatibility

Protocol structs include explicit version fields. A future version must define its own canonical rules and activation conditions; parsers must not silently reinterpret unknown versions as v1.

The foundation crate should avoid encoding node policy into primitive types. Consensus, chain selection, PoW, mempool policy, P2P, and wallet behavior remain separate modules so they can evolve without changing canonical object bytes unnecessarily.

## Acceptance boundary

This design is complete when the repository contains a clean Rust workspace and `oregon-primitives` can deterministically encode, decode, hash, and test the v0 primitive objects and golden vectors described above.

It does not claim Oregon is a runnable blockchain yet. The next architectural specifications will cover monetary/coinbase consensus and PoW/difficulty, followed by chain state, storage, networking, mining, RPC, and wallet layers.
