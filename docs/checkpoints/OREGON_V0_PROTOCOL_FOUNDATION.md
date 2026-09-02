# Oregon Protocol v0 Foundation Acceptance

Date: 2026-09-02
Development branch: `oregon-v0-protocol`

Design specification: `docs/superpowers/specs/2026-09-02-oregon-v0-protocol-design.md`
Implementation plan: `docs/superpowers/plans/2026-09-02-oregon-v0-protocol-foundation.md`
Golden vectors: `tests/vectors/protocol-v0.json`
Progress record: `docs/checkpoints/OREGON_V0_PROTOCOL_PROGRESS.md`

## Accepted scope

This checkpoint accepts the independent Rust protocol-foundation layer only. It freezes the primitive data model, canonical bytes, identifier domains, bounded decoders, transaction commitment tree, block-header container, and deterministic protocol-v0 golden vectors.

It does not claim that Oregon is a runnable blockchain or production network.

## Toolchain and workspace

- Rust: 1.85.0
- Edition: 2024
- Workspace crate: `crates/oregon-primitives`
- Production dependencies: `blake3`, `thiserror`
- Test/reproducibility dependencies: `proptest`, `serde`, `serde_json`

## Monetary representation

- Native unit: OREG
- `1 OREG = 100,000,000` base units
- Maximum supply envelope: `100,000,000,000,000` base units = `1,000,000 OREG`
- Public founder-allocation constant: `5,000,000,000,000` base units = `50,000 OREG` = 5%
- Amount arithmetic is integer-only and checked for overflow, underflow, and values above the supply envelope.

The previously approved mining design parameters remain design inputs for later consensus work: initial subsidy 2.375 OREG, halving interval 200,000 blocks, and target block interval 300 seconds. This foundation checkpoint does not claim that the final emission engine or difficulty algorithm is implemented.

## Protocol-v0 object identity

All object identifiers use 256-bit BLAKE3 output with explicit domain separation:

- Transaction ID: `OREGON/TX/V0\0`
- Block-header ID: `OREGON/BLOCK/V0\0`
- Merkle leaf: `OREGON/MERKLE-LEAF/V0\0`
- Merkle internal node: `OREGON/MERKLE/V0\0`

Transaction IDs commit to canonical transaction bytes including witness bytes.

Block IDs commit only to canonical block-header bytes. Transaction contents are committed through `transaction_root`.

## Canonical encoding

- Consensus integers are little-endian.
- Fixed-width fields use their exact widths.
- Collection and byte-string lengths use Oregon canonical varints.
- Non-minimal varints are rejected.
- Complete-object decoders reject trailing bytes.
- Truncation and configured allocation-limit violations return typed errors rather than panicking.
- Generic binary codecs such as bincode/postcard are not part of consensus encoding.

Default defensive limits include 65,535 transaction inputs, 65,535 outputs, 1,024 witness items per input, 1 MiB per witness item, 64 KiB locking programs, 1,000,000 transactions per decoded block, and 64 MiB maximum complete-object bytes.

## Transaction commitment tree

Merkle leaves commit to ordered transaction IDs using the Oregon leaf domain. Internal nodes hash ordered left/right children using the Oregon internal-node domain.

For odd levels, the final node is promoted unchanged. It is not duplicated.

An empty transaction list is invalid for a block; no empty-root constant exists.

## Block primitive

Canonical block-header layout is exactly 114 bytes:

```text
version:u16
previous_block:[u8;32]
transaction_root:[u8;32]
timestamp:u64
difficulty_commitment:[u8;32]
nonce:u64
```

`difficulty_commitment` is intentionally opaque in this foundation and reserves a fixed commitment slot for the future reviewed PoW/difficulty design.

Block encoding is the fixed-width header, transaction-count canonical varint, then canonical transaction encodings.

## Golden protocol artifact

`tests/vectors/protocol-v0.json` freezes deterministic examples for:

- canonical varint boundaries;
- explicit non-minimal varint rejection cases;
- minimum valid version-1 transaction;
- multi-input/output/witness transaction;
- exact canonical transaction bytes and TxIDs;
- one-, two-, and three-transaction Merkle roots;
- odd-node promotion behavior;
- exact canonical block-header bytes and block ID;
- maximum and above-maximum amount boundaries.

The checked-in JSON is the protocol artifact. `crates/oregon-primitives/examples/generate_protocol_v0.rs` and its narrow workflow exist only to make that deterministic fixture reproducible and reviewable.

## Verification evidence

Fresh full acceptance gate before this checkpoint-document commit:

```text
cargo +1.85.0 test --workspace --all-targets
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --workspace --all-targets -- -D warnings
```

GitHub Actions run `33667003179` on commit `aed2e932a485f1e987b3024e3cc657e7c3ad544b` completed successfully for all three gates.

Task-6 golden-vector gate also completed successfully in GitHub Actions run `33666671802` on commit `29576e09948670853ca24c3edf40b529ccc8b60a`.

## Mutation-sensitivity evidence

A throwaway branch, `oregon-v0-mutation-odd-merkle-2026-09-02`, intentionally replaced odd-node promotion with duplicate-last hashing. GitHub Actions run `33666851003` failed specifically at:

`merkle::tests::three_transaction_root_promotes_last_leaf_without_duplication`

The mutation branch produced 43 passing unit/property tests and one expected Merkle failure before the job stopped. The development branch was never mutated. This proves the accepted test suite is sensitive to that consensus-critical Merkle rule.

## Independence review

The accepted Rust workspace was reviewed separately from the historical Bitcoin-Core patch experiments.

- Recursive tree review at development head showed no `.cpp` or `.h` files in the Rust foundation tree.
- Workspace membership contains only `crates/oregon-primitives`.
- Cargo manifests contain no Bitcoin crate dependency, bincode, or postcard.
- Reviewed Rust implementation/test/generator sources contain no `bitcoin::` implementation import and no `unsafe` block.
- Historical documentation may mention Bitcoin/Core for project history or comparison; those references are not implementation coupling in the accepted Rust foundation.

## Explicit non-goals

This checkpoint does not include or claim:

- a runnable P2P node;
- peer discovery or networking;
- production genesis blocks or chain parameters;
- final proof-of-work function;
- final difficulty-retarget algorithm;
- UTXO state-transition validation;
- signature, script, or locking-program execution semantics;
- coinbase/founder-allocation enforcement in the new Rust consensus engine;
- final mining emission engine;
- mempool or block assembly policy;
- wallet/key management;
- mining RPC;
- production founder private keys, seeds, or credentials.

Those areas require later reviewed milestones and must not be inferred from this protocol-foundation acceptance.

## Acceptance rule

Any future change to canonical bytes, hash domains, transaction identity, Merkle odd-node behavior, block-header layout, monetary constants, or golden-vector values is a protocol change. Such changes require explicit review, updated vectors, mutation-sensitive tests where applicable, and a new checkpoint before production use.
