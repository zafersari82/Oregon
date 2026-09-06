# Oregon Contract State and Commitments V1 Design

**Status:** Owner-approved Stage 2 design; inactive until later activation and main-integration decisions

**Date:** 2026-09-05

**Base:** `b97f9d3af9e2c9c4011750cfb69cce8fd9117a8a`

**Parent contracts:**
- `docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md`
- `docs/architecture/OREGON_PLATFORM_ARCHITECTURE_CONTRACT.md`
- `docs/superpowers/specs/2026-09-05-execution-architecture-design.md`
- `docs/superpowers/specs/2026-09-05-execution-envelope-wire-v1.md`
- `docs/checkpoints/OREGON_EXECUTION_ENVELOPE_PROGRESS.md`

## 1. Purpose and authority

This design freezes Execution Architecture V1 §27 Stage 2: logical contract state, versioned child commitments, and the Oregon aggregate state commitment framework.

The owner explicitly delegated the remaining Stage 2 technical selections to the lead architecture assistant and requested that future low-level approval prompts not block progress. The selections below therefore become the owner-approved V1 direction for this subsystem once committed to the repository. This does not grant permission to alter already-frozen M0–M6 or execution architecture contracts.

This stage remains inactive. It must not alter accepted `BlockHeader` bytes, block ids, current transactions, mempool admission, native UTXO semantics, RocksDB schema, chain selection, P2P behavior, RPC behavior, or wallet behavior.

## 2. Scope and non-goals

Stage 2 delivers:

1. canonical commitment-domain and commitment-scheme identifiers;
2. canonical `domain_id + scheme_id + root` descriptors;
3. one canonical Oregon aggregate commitment over strictly ordered child descriptors;
4. a storage-independent fixed-depth 256-bit Sparse Merkle Tree commitment scheme for Oregon-owned key/value state domains;
5. immutable/content-addressed logical node records and value commitments;
6. canonical write-set normalization;
7. membership and non-membership proof construction/verification interfaces;
8. snapshot and state-transition outputs suitable for later transaction journals and durable persistence;
9. deterministic golden vectors and mutation gates.

Stage 2 does **not**:

- modify the current 114-byte `BlockHeader`;
- add `state_commitment_root` to a block header yet;
- define EVM trie/proof bytes;
- implement an EVM or WASM interpreter;
- activate contracts;
- add RocksDB column families or migrations;
- modify native UTXO validity or supply accounting;
- implement execution fees, receipts, async message execution, RPC, bridge, privacy, oracle, or AI truth.

## 3. Ownership boundaries

### 3.1 `oregon-primitives`

Owns:

- `CommitmentDomainId`;
- `CommitmentSchemeId`;
- `StateCommitmentDescriptor`;
- `StateCommitmentSetV1` canonical descriptor-set encoding;
- aggregate-root hashing;
- decode/resource bounds for those wire-like primitives;
- the public Oregon `domain_hash` helper used by lower execution-state crates.

It does not own logical state mutation, node persistence, VM semantics, or consensus activation policy.

### 3.2 `oregon-contract-state`

A new crate owns:

- Oregon Sparse Merkle Tree V1 logical semantics;
- key/value commitments;
- domain-specific empty-root ladders;
- immutable logical node records;
- canonical write-set normalization;
- snapshots and root transitions;
- proof construction and proof verification;
- storage-neutral node/value reader interfaces.

It must not depend on RocksDB or `oregon-storage`.

### 3.3 `oregon-storage`

Remains the only owner of RocksDB column families, physical codecs, migrations, write batches, WAL/sync durability, pruning mechanics, and corruption/recovery storage policy.

Stage 2 does not change its schema. Later persistence work may add contract-state records through an explicit schema migration while preserving `DurabilityMode::Sync` for accepted-state publication.

### 3.4 `oregon-chainstate`

Later composes native UTXO and execution-domain transitions atomically and publishes a new active root set only after durable storage succeeds. Stage 2 does not change current chainstate behavior.

### 3.5 EVM and WASM separation

EVM and WASM never share raw account/storage databases.

- Oregon/WASM-owned logical key/value state uses Oregon Sparse Merkle V1 when activated for that domain.
- EVM state remains owned by the future `oregon-vm-evm` backend and publishes a typed EVM state commitment descriptor.
- The exact EVM commitment algorithm/proof bytes are frozen only in the EVM activation design. Stage 2 reserves the descriptor scheme id but does not implement or simulate Ethereum state.

## 4. Commitment domain identifiers

`CommitmentDomainId` is a closed `u16` little-endian V1 identifier namespace, independent from execution-domain and execution-address namespaces.

V1 assignments:

| Value | Domain |
| --- | --- |
| `0x0001` | Native UTXO state |
| `0x0010` | EVM state |
| `0x0011` | WASM state |
| `0x0020` | Execution accounting state |
| `0x0030` | Execution receipts |
| `0x0040` | Asynchronous outbox |
| `0x0041` | Asynchronous consumed-message state |
| `0x0050` | Execution fee/base-fee state |

Unknown V1 domain ids fail closed until supported by a future explicitly activated software version.

The presence of an id in this table reserves identity only. It does not activate that domain or select its commitment scheme.

## 5. Commitment scheme identifiers

`CommitmentSchemeId` is a closed `u16` little-endian V1 identifier namespace.

V1 assignments:

| Value | Scheme |
| --- | --- |
| `0x0001` | `OREGON_SMT_V1` — Oregon fixed-depth Sparse Merkle Tree V1 |
| `0x0100` | `EVM_COMMITMENT_V1` — reserved typed EVM state commitment; exact trie/proof semantics frozen later |

Unknown V1 scheme ids fail closed until supported by a future explicitly activated software version.

`OREGON_SMT_V1` is implemented in Stage 2. `EVM_COMMITMENT_V1` is only a reserved descriptor identity in Stage 2 and must not be accepted as proof that EVM execution exists.

## 6. Canonical child commitment descriptor

A `StateCommitmentDescriptor` is exactly 36 bytes:

1. `domain_id: u16 LE`
2. `commitment_scheme_id: u16 LE`
3. `state_root: [u8; 32]`

The descriptor contains no flags, length prefixes, optional fields, or trailing bytes.

Construction requires known V1 domain/scheme ids. Domain/scheme compatibility remains a later `oregon-consensus` activation rule; structural primitives do not silently reinterpret a domain because a scheme is known.

## 7. Canonical Oregon aggregate state commitment

`StateCommitmentSetV1` is the canonical ordered container used to derive the future header-level `state_commitment_root`.

### 7.1 Encoding

The canonical bytes are:

1. `aggregate_version: u16 LE = 1`
2. one or more 36-byte `StateCommitmentDescriptor` values concatenated with no count field.

Descriptor count is derived exactly from the total remaining byte length.

Hard ceiling:

- `MAX_STATE_COMMITMENTS = 32`.

Decoder requirements:

- total bytes must be at least 38 bytes (`version + one descriptor`);
- version must be exactly 1;
- bytes after the version must be an exact multiple of 36;
- descriptor count must be `1..=32`;
- descriptors must be strictly increasing by numeric `domain_id`;
- duplicate domains are invalid even if scheme/root differ;
- unknown ids fail closed;
- trailing bytes are impossible by construction and malformed length is rejected.

### 7.2 Aggregate root

The root is:

`BLAKE3("OREGON/STATE/AGGREGATE/V1\0" || canonical_state_commitment_set_bytes)`.

The canonical bytes commit the aggregate version, every domain id, every scheme id, every child root, and canonical ordering.

Changing only a scheme id must change the aggregate root.

### 7.3 Activation boundary

Stage 2 computes aggregate roots as inactive primitives only.

The current `BlockHeader` remains byte-for-byte unchanged. A future explicit header-version/activation design will add one aggregate `state_commitment_root`; it will not add one header field per VM/domain.

## 8. Oregon Sparse Merkle Tree V1

`OREGON_SMT_V1` is selected for Oregon-owned deterministic key/value commitment domains, including WASM state and execution-owned state families when those domains are activated.

It is not used to force EVM state into Oregon storage semantics.

### 8.1 Fixed depth and path order

- Tree depth is exactly 256 bits.
- Root internal depth is `0`.
- Leaf depth is `256`.
- Path bit at depth `d` is read MSB-first:

`bit(d) = (path_key[d / 8] >> (7 - (d % 8))) & 1` for `d in 0..256`.

A `0` bit selects left and `1` selects right.

Implementations may optimize storage/layout but may not change logical depth, path order, or hash inputs.

## 9. Resource ceilings

Structural Stage 2 ceilings:

- `MAX_STATE_KEY_BYTES = 1_024`;
- `MAX_STATE_VALUE_BYTES = 1_048_576`;
- `MAX_STATE_WRITE_SET_ENTRIES = 65_536`;
- `MAX_STATE_COMMITMENTS = 32`;
- `SMT_PROOF_BITMAP_BYTES = 32`;
- `MAX_SMT_SIBLINGS = 256`;
- `MAX_SMT_PROOF_BYTES = 8_226` (`2-byte proof version + 32-byte bitmap + 256 * 32-byte sibling hashes`).

These are hard structural safety ceilings, not promised block/transaction economic limits. Later normalized-weight consensus rules may impose much lower limits. Raising these structural limits after activation requires explicit versioned review; they are never runtime admin settings.

All lengths are checked before allocation/copy. Checked arithmetic is mandatory.

## 10. Domain-separated SMT hashing

All hashes are 256-bit BLAKE3 using Oregon's explicit domain-hash pattern.

The existing `oregon-primitives::hash::domain_hash` behavior is made public/re-exported rather than reimplemented in `oregon-contract-state`.

Exact domains:

- key: `OREGON/STATE/SMT/KEY/V1\0`
- value: `OREGON/STATE/SMT/VALUE/V1\0`
- empty leaf: `OREGON/STATE/SMT/EMPTY/V1\0`
- populated leaf: `OREGON/STATE/SMT/LEAF/V1\0`
- internal node: `OREGON/STATE/SMT/NODE/V1\0`

`domain_id` is encoded as exactly two little-endian bytes in every SMT hash payload.

### 10.1 Path key

For canonical logical key bytes `K`:

`path_key = H(KEY_DOMAIN, domain_id_le || K)`.

The raw key length is checked before hashing. No Unicode/string normalization occurs in the SMT layer; callers supply canonical domain-specific bytes.

### 10.2 Value hash

For present canonical value bytes `V`:

`value_hash = H(VALUE_DOMAIN, domain_id_le || V)`.

`Some([])` is a real present empty value and therefore has a real value hash.

`None` means absence/deletion and is not encoded as an empty byte vector.

### 10.3 Empty leaf and empty ladder

For domain `D`:

`empty[256] = H(EMPTY_DOMAIN, domain_id_le)`.

For `depth = 255` down to `0`:

`empty[depth] = H(NODE_DOMAIN, domain_id_le || depth_u16_le || empty[depth + 1] || empty[depth + 1])`.

The empty state root is exactly `empty[0]`.

No hard-coded magic zero root is permitted. Golden vectors pin the complete construction.

### 10.4 Populated leaf

A populated leaf hash is:

`leaf_hash = H(LEAF_DOMAIN, domain_id_le || path_key || value_hash)`.

The logical immutable leaf record is:

- `path_key: Hash256`
- `value_hash: Hash256`

A non-empty leaf record is content-addressed by `leaf_hash`.

### 10.5 Internal node

For internal `depth in 0..=255`:

`node_hash = H(NODE_DOMAIN, domain_id_le || depth_u16_le || left_hash || right_hash)`.

The logical immutable branch record is:

- `depth: u16`
- `left: Hash256`
- `right: Hash256`

Depth is part of the commitment to prevent an identical child pair from being reinterpreted at another tree level.

A branch whose left and right children both equal `empty[depth + 1]` is non-canonical and must collapse to `empty[depth]`; it is not stored as an explicit node.

## 11. Immutable/content-addressed logical state

Stage 2 chooses persistent immutable nodes rather than in-place tree mutation.

- Non-default branch and leaf nodes are addressed by their computed node hash.
- Present value blobs are addressed by `value_hash`.
- Default empty nodes are deterministic and are not stored.
- Applying writes produces a new root plus append-only logical node/value outputs.
- The previous root remains valid as long as its referenced content-addressed records remain retained.

This makes snapshots and reorg rollback root-pointer operations rather than inverse-patch replay inside the SMT.

Physical retention, garbage collection, pruning and RocksDB codecs remain `oregon-storage` responsibilities in later work.

## 12. Storage-neutral reader contract

`oregon-contract-state` defines storage-neutral read interfaces equivalent to:

```rust
pub trait StateSource {
    fn get_node(&self, node_hash: Hash256) -> Result<Option<StateNode>, StateError>;
    fn get_value(&self, value_hash: Hash256) -> Result<Option<Vec<u8>>, StateError>;
}
```

Exact Rust ownership/borrowing details may be adjusted in the implementation plan without changing semantics.

Rules:

- requesting a deterministic empty hash must not require a stored node;
- a referenced non-empty node missing from the source is corruption, never implicit empty state;
- a returned node whose recomputed hash differs from the requested hash is corruption;
- a membership leaf whose referenced value blob is missing or hashes differently is corruption;
- proof verification itself requires no `StateSource` and performs no database access.

## 13. Snapshot and transition model

A domain snapshot is logically:

- `domain_id`;
- `scheme_id = OREGON_SMT_V1`;
- `root`.

Applying a finalized write set to a snapshot returns a transition containing at least:

- `domain_id`;
- `old_root`;
- `new_root`;
- newly created immutable node records keyed by hash;
- newly introduced value blobs keyed by value hash.

The transition does not mutate the source and does not publish itself as chain state.

A later transaction/block journal may discard a transition on revert. A later chainstate/storage layer may durably publish its records and root pointer atomically.

## 14. Canonical write sets

A finalized state write is logically:

- canonical raw `key: Vec<u8>`;
- `value: Option<Vec<u8>>`, where `None` means delete.

Before applying:

1. validate key/value byte ceilings;
2. derive every `path_key`;
3. sort writes strictly ascending by the 32-byte `path_key`;
4. reject duplicate `path_key` values;
5. reject more than `MAX_STATE_WRITE_SET_ENTRIES`.

A transaction journal may assign the same logical key repeatedly before finalization. The finalized commitment-layer write set contains only the final value for that logical key.

Different raw keys that produce the same 256-bit `path_key` are treated as a commitment collision and fail closed through duplicate-path rejection.

Deleting an absent key is a deterministic no-op. Setting a key to its already committed value is a deterministic no-op. No-op writes remain subject to later resource charging policy.

## 15. Applying a write set

Logical application requirements:

- result root must be independent of input iteration order because finalization canonicalizes by path key;
- implementation may use a batch algorithm or sequential overlay, but final roots/nodes must match golden vectors;
- intermediate newly created nodes may be resolved from an in-memory overlay before the base `StateSource`;
- no attacker-controlled `Vec::with_capacity` or unchecked multiplication based on unbounded input;
- missing/corrupt referenced source records abort the transition;
- no partial transition is published on error.

## 16. Canonical Sparse Merkle proof V1

The same proof type supports membership and non-membership.

The verifier receives externally:

- `domain_id`;
- canonical raw key bytes;
- `Option<&[u8]>` expected value (`Some` membership, `None` non-membership);
- proof bytes/object;
- expected root.

### 16.1 Wire encoding

Canonical proof bytes are:

1. `proof_version: u16 LE = 1`
2. `sibling_bitmap: [u8; 32]`
3. exactly one 32-byte sibling hash for every set bit in the bitmap, concatenated in increasing tree-depth order (`0` through `255`).

There is no sibling count field; count is exactly the bitmap popcount.

Bitmap bit for depth `d` is MSB-first:

`bitmap[d / 8] & (0x80 >> (d % 8))`.

Maximum bytes are `8_226`.

### 16.2 Canonical compression rule

At depth `d`, the omitted/default sibling is exactly `empty[d + 1]` for the proof domain.

- bitmap bit `0`: verifier substitutes `empty[d + 1]`;
- bitmap bit `1`: one explicit sibling hash must be present;
- an explicit sibling equal to `empty[d + 1]` is non-canonical and rejected;
- malformed byte length, unsupported version or trailing bytes are rejected.

This gives one canonical proof encoding for a given proof path while avoiding a mandatory 8 KiB proof when most siblings are default.

### 16.3 Verification

Verifier:

1. validates key/value/proof ceilings;
2. derives `path_key` and optional `value_hash`;
3. begins with populated leaf hash for membership or `empty[256]` for non-membership;
4. reconstructs upward from depth `255` to `0`, selecting left/right using the MSB-first path bit and consuming the depth's sibling;
5. succeeds only if the reconstructed root equals the expected root.

Wrong domain, wrong key, wrong value, wrong bitmap, wrong sibling, or wrong root must fail.

Proof verification never reads RocksDB or any mutable global state.

## 17. Proof construction

Proof construction walks the authoritative snapshot root through `StateSource`.

For each depth, it records the sibling only when that sibling differs from the deterministic default `empty[depth + 1]`.

At leaf depth:

- membership requires the leaf path to equal the requested `path_key` and the referenced value blob to verify;
- non-membership requires the path to resolve to the deterministic empty leaf;
- any missing/corrupt non-empty node/value fails closed.

The emitted proof must round-trip through canonical proof decoding and independent verification.

## 18. Multi-domain snapshots and aggregate commitments

EVM, WASM and execution-owned state roots remain separate child roots.

A future block execution result gathers the activated child descriptors, sorts them by domain id, validates the activated scheme policy, and derives one aggregate Oregon `state_commitment_root`.

Child roots remain independently provable:

1. prove the child descriptor against the canonical aggregate descriptor set/root as required by the future header-proof format;
2. then use the child domain's declared scheme-specific proof.

Stage 2 only freezes child descriptor bytes and aggregate root construction. Header inclusion/proof packaging is activated later.

## 19. Reorg, rollback and crash-recovery semantics

The logical SMT never performs destructive in-place rollback.

- A snapshot is identified by its root.
- A reverted transaction discards its unpublished transition.
- A chain reorg selects the prior durable domain-root set and later chainstate replays the winning branch.
- Unreachable immutable nodes/value blobs may remain until storage-owned pruning/GC proves them safe to remove.
- Missing retained content required by a supposedly durable root is corruption and must produce fail-closed recovery/reindex behavior; it is never silently treated as empty state.

When execution persistence is later integrated, UTXO changes, execution node/value writes, active domain roots, block index/tip updates and health metadata must be included in the same authoritative durable acceptance boundary before a block is reported accepted.

## 20. EVM compatibility boundary

Stage 2 intentionally does not clone Ethereum's state trie for Oregon/WASM state.

`EVM_COMMITMENT_V1` remains a distinct scheme descriptor. The future EVM design will freeze the exact upstream revision, root algorithm, account/storage encoding and proof compatibility rules required for supported Ethereum tooling.

An EVM root is never recomputed by `OREGON_SMT_V1`, and a WASM/Oregon SMT root is never presented as an Ethereum state root.

Both later contribute typed descriptors to the same Oregon aggregate commitment.

## 21. Current header and transaction bytes remain frozen

Stage 2 must not modify:

- `crates/oregon-primitives/src/block.rs` header fields or 114-byte encoding;
- current block-id hashing;
- `crates/oregon-primitives/src/transaction.rs` encoding/txid;
- M0–M6 block/mempool/chainstate activation paths.

The new commitment primitives and crate are inactive library code only.

## 22. Error handling

Stage 2 uses explicit typed errors and fail-closed behavior for at least:

- unknown commitment domain/scheme;
- unsupported aggregate/proof version;
- empty or oversized commitment set;
- malformed descriptor-set length;
- non-canonical descriptor ordering;
- duplicate commitment domain;
- oversized key/value/write set/proof;
- duplicate/colliding path key;
- malformed proof length;
- redundant default sibling;
- missing/corrupt node;
- node hash mismatch;
- node depth mismatch;
- missing/corrupt value blob;
- invalid membership/non-membership proof;
- checked-arithmetic overflow.

No panic, implicit empty substitution, or partial publication is accepted for hostile input/corrupt storage.

## 23. Golden-vector requirements

Literal vectors must pin at minimum:

### Commitment descriptors/aggregate

- exact descriptor bytes for every currently known id;
- one-descriptor aggregate root;
- multiple descriptors in canonical order;
- changing a domain id changes aggregate root;
- changing a scheme id changes aggregate root;
- changing a child root changes aggregate root;
- reversed order and duplicate domain rejection.

### Oregon SMT

For at least WASM and one execution-owned domain:

- path-key hash for literal key;
- value hash for literal value;
- present-empty value hash;
- domain-specific empty root;
- one-leaf root;
- two-leaf root on paths diverging near the root;
- two-leaf root on paths sharing a long prefix;
- update existing value;
- delete existing value back to prior/empty root;
- delete absent no-op;
- same raw key/value under a different domain yields a different root.

### Proofs

- literal membership proof bytes;
- literal non-membership proof bytes;
- proof with zero explicit siblings when possible;
- proof with multiple non-default siblings;
- wrong key/value/domain/root rejection;
- redundant default sibling rejection;
- truncation/trailing/malformed bitmap-length rejection.

Golden expected hashes/bytes are literal independent values, not generated at test runtime by the production implementation under test.

## 24. Required mutation gates

Stage 2 mutation testing must kill at least these fail-open/consensus mutants:

1. omit `domain_id` from SMT key commitment;
2. use LSB-first instead of MSB-first path bits;
3. omit internal `depth` from node commitment;
4. treat `Some([])` as deletion/absence;
5. accept duplicate/colliding path keys;
6. substitute a missing non-empty node with an empty node;
7. accept redundant explicit default proof sibling;
8. omit `scheme_id` from aggregate commitment identity;
9. accept unsorted/duplicate aggregate descriptors;
10. accept a proof under the wrong commitment domain.

A mutation is killed only when the intended named test fails. Compilation failure alone is not a valid kill.

## 25. Implementation sequence

Implementation follows test-first isolated development:

1. primitives commitment descriptor/aggregate vectors and expected-red tests;
2. canonical descriptor/aggregate implementation;
3. new `oregon-contract-state` crate and literal SMT vectors/tests expected-red;
4. hashing/empty ladder/node model;
5. write-set normalization and immutable transition engine;
6. proof codec/construction/verification;
7. security/adversarial tests and mutation runner;
8. full inherited workspace/RandomX gates;
9. persistent progress checkpoint and `HANDOFF.md` update.

No `main` integration or activation occurs as part of Stage 2 implementation.

## 26. Stage 2 completion criteria

Stage 2 is complete only when:

- the spec and implementation plan are committed;
- test-only contracts first demonstrate intended expected-red CI;
- all descriptor/aggregate/SMT/proof golden vectors pass;
- hostile/truncation/resource-bound tests pass;
- every required mutation is killed by its intended test;
- full workspace tests, docs, rustfmt and Clippy pass on the exact implementation head;
- inherited RandomX architecture/full-light parity remains green where `oregon-primitives` changed;
- a checkpoint records exact SHAs/trees/run ids and non-activation limits;
- `HANDOFF.md` points to Stage 3 rather than repeating completed Stage 2 work;
- `main` remains unchanged pending a separate integration decision.
