# Oregon PoW / Consensus v1 Design

Date: 2026-09-02
Status: design for review; no implementation authorization until explicit user approval
Base checkpoint: `oregon-v0-checkpoint-foundation-accepted-2026-09-02`
Base commit: `033160dafd4c2a74cd6dcfa2bb7b628c3cab499c`
Design branch: `oregon-v1-consensus-design`

## 1. Purpose and scope

This document defines the next Oregon protocol layer above the accepted v0 primitive foundation. It specifies the proof-of-work family, difficulty adjustment, monetary issuance, one-time founder allocation, coinbase rules, UTXO state transition, v1 payment authorization, address/network identity, deterministic mainnet genesis ceremony, and the architectural boundaries for the first full node.

The design keeps Oregon an independent Rust protocol. Bitcoin, Bitcoin Cash, Monero/RandomX, and other systems are reference sources for reviewed mechanisms, not source-code baselines for Oregon consensus.

This document intentionally does not authorize implementation. After this document is reviewed and approved, implementation must be decomposed into milestone plans and executed test-first.

## 2. Existing frozen foundation

The following v0 decisions remain unchanged:

- Rust edition 2024, MSRV 1.85.0.
- UTXO transaction model.
- `1 OREG = 100,000,000` base units.
- Maximum supply envelope: `1,000,000 OREG = 100,000,000,000,000` base units.
- Public founder-allocation constant: `50,000 OREG = 5,000,000,000,000` base units.
- Transaction IDs are domain-separated BLAKE3 over canonical transaction bytes and include witness bytes.
- Block IDs are domain-separated BLAKE3 over the canonical 114-byte block header.
- Transaction Merkle leaves and internal nodes use separate Oregon BLAKE3 domains.
- Odd Merkle nodes are promoted unchanged, not duplicated.
- Consensus binary encoding is explicit and canonical.
- Generic consensus serialization libraries such as bincode/postcard are not permitted.
- Defensive decode limits are distinct from economic/consensus block limits.

Any change to these items requires a new protocol-change review and updated golden vectors.

## 3. Normative conventions

`MUST`, `MUST NOT`, `SHALL`, `SHALL NOT`, `SHOULD`, and `MAY` are normative.

Consensus arithmetic MUST use deterministic integer operations. Floating-point arithmetic MUST NOT influence block validity, difficulty, monetary issuance, target comparison, fees, or signature digests.

Where an integer byte order is specified as little-endian, implementations MUST interpret exactly that byte order regardless of host architecture.

## 4. Consensus constants

### 4.1 Fixed v1 constants

- Target block interval: `300` seconds.
- ASERT half-life: `21,600` seconds (6 hours).
- ASERT fixed-point radix: `65,536` (`2^16`).
- RandomX key epoch: `864` Oregon blocks (approximately 3 days).
- RandomX key activation delay: `24` Oregon blocks (approximately 2 hours).
- Coinbase maturity: `120` blocks (approximately 10 hours).
- Initial normal block subsidy: `237,500,000` base units = `2.37500000 OREG`.
- Halving interval: `200,000` blocks.
- Maximum canonical block size: `1,048,576` bytes (1 MiB).
- Maximum canonical size of each transaction in a non-genesis block, including coinbase: `102,400` bytes (100 KiB).
- Mainnet address HRP: `oreg`.
- Testnet address HRP: `toreg`.
- Dev/regtest address HRP: `doreg`.
- KeyCommit locking-program version byte: `0x01`.

### 4.2 Launch-profile parameters

The algorithm accepts the following chain-profile values, but mainnet MUST NOT launch until an explicit benchmark/launch checkpoint freezes their exact bytes:

- `POW_LIMIT`: the easiest permitted non-zero 256-bit target.
- `INITIAL_TARGET`: the exact target required for height 1.
- `FOUNDER_KEY_COMMITMENT`: the announced 32-byte founder key commitment.
- Bitcoin fair-launch anchor height and resulting anchor/confirmation data.
- Deterministic genesis block bytes and resulting genesis block ID.
- Derived mainnet chain ID and P2P magic.
- Network ports, seed hosts, and initial peer-discovery profile.

`POW_LIMIT` and `INITIAL_TARGET` are deliberate launch parameters, not implementation placeholders. They are withheld from numeric freezing until public CPU benchmarks exist. The mainnet launch gate MUST bind exact 32-byte values before genesis can be produced.

## 5. Proof of work

### 5.1 PoW family

Oregon PoW v1 SHALL use the RandomX v2 family.

The upstream reference baseline is RandomX tag `v2.0.1`, commit:

`aaafe71322df6602c21a5c72937ac284724ae561`

No implementation may silently follow upstream `master` or an unpinned release range.

The only planned Oregon-specific RandomX configuration change is:

`RANDOMX_ARGON_SALT = "OREGON-RANDOMX-V1"`

All other RandomX v2.0.1 consensus-affecting configuration constants SHALL initially remain at the upstream v2.0.1 defaults. Any additional RandomX parameter change is a protocol change and requires benchmark evidence, cross-platform test vectors, and explicit review.

The vendored/upstream source plus the Oregon configuration patch MUST be checksum-pinned by the implementation checkpoint.

### 5.2 CPU-accessibility goal

RandomX is selected to make ordinary general-purpose CPUs viable participants and to raise the cost of specialized hardware advantage. Oregon MUST NOT claim that ASICs or botnets are mathematically impossible.

Consensus MUST NOT attempt to distinguish a legitimate home CPU from a botnet CPU through hardware attestation, device identity, miner licenses, IP identity, or privileged enrollment.

### 5.3 RandomX key schedule

For candidate block height `h`, define the RandomX key block height:

```text
if h < 888:
    key_block_height = 0
else:
    key_block_height = floor((h - 24) / 864) * 864
```

Height `0` means the genesis block.

The RandomX key is exactly:

```text
K = BLAKE3("OREGON/RANDOMX-KEY/V1\0" || key_block_id_bytes)
```

where `key_block_id_bytes` is the internal 32-byte Oregon block ID representation.

The miner MUST NOT select the key. A reorganization that changes the selected key block necessarily changes the key.

Node implementations SHOULD maintain at most the cache states needed for previous, active, and pending key epochs. Cache policy is not consensus; the resulting RandomX hash is consensus.

### 5.4 PoW input and comparison

The RandomX input is:

```text
"OREGON/POW/V1\0" || canonical_114_byte_block_header
```

The 32-byte RandomX output is interpreted as an unsigned 256-bit little-endian integer:

```text
pow_value = U256::from_le_bytes(randomx_output)
```

A header satisfies PoW exactly when:

```text
pow_value <= target
```

where `target` is the block's validated 256-bit target.

Fast/full-memory mining and light verification MUST produce identical 32-byte hashes for identical key/input pairs. Cross-mode and cross-architecture vectors are mandatory acceptance tests.

### 5.5 RandomX runtime safety

A JIT-enabled implementation SHOULD enable RandomX secure/W^X behavior where supported. This is an implementation-security requirement, not a consensus fork condition.

Validators SHOULD default to light verification unless the operator opts into full dataset mode. Miners SHOULD use the full dataset when hardware permits.

## 6. Target representation and ASERT difficulty

### 6.1 Full-width target commitment

The existing 32-byte `difficulty_commitment` header field SHALL become the exact Oregon v1 target.

It is encoded as a canonical 32-byte unsigned little-endian integer. There is no compact `nBits` representation and therefore no compact-target mantissa/exponent rounding rule.

A valid target MUST satisfy:

```text
1 <= target <= POW_LIMIT
```

The header's `difficulty_commitment` MUST equal the target independently computed from the parent chain. A miner cannot choose an easier target.

### 6.2 Height 1

Height `1` MUST use exactly `INITIAL_TARGET`.

Genesis carries `INITIAL_TARGET` in its opaque difficulty field for deterministic chain-profile commitment, but genesis itself is a hardcoded anchor and is not RandomX-mined.

### 6.3 ASERT anchor

For every height `h >= 2`, Oregon computes the required target from a fixed ASERT anchor:

- anchor block height: `1`
- anchor target: `INITIAL_TARGET`
- anchor-parent timestamp: genesis timestamp
- evaluation block: the parent of the candidate block

For a candidate at height `h`, define:

```text
h_eval = h - 1
t_eval = timestamp(parent)
h_ref = 1
t_ref = timestamp(genesis)
target_ref = INITIAL_TARGET
height_delta = h_eval - h_ref
ideal_block_time = 300
halflife = 21600
radix = 65536
```

The ideal mathematical target is:

```text
next_target = target_ref * 2^(
    (t_eval - t_ref - ideal_block_time * (height_delta + 1)) / halflife
)
```

Consensus MUST implement this with deterministic integer fixed-point arithmetic, not floating point.

### 6.4 Oregon ASERT integer algorithm

Oregon SHALL adapt the reviewed `aserti3` fixed-point polynomial while operating directly on full 256-bit targets.

Pseudo-code:

```text
time_delta = signed(t_eval) - signed(t_ref)
height_delta = h_eval - h_ref

exponent = trunc_div(
    (time_delta - 300 * (height_delta + 1)) * 65536,
    21600
)

num_shifts = arithmetic_shift_right(exponent, 16)
frac = exponent - num_shifts * 65536

factor = arithmetic_shift_right(
      195766423245049 * frac
    + 971821376 * frac^2
    + 5127 * frac^3
    + 2^47,
    48
) + 65536

candidate = target_ref * factor

if num_shifts < 0:
    candidate = candidate >> (-num_shifts)
else:
    candidate = candidate << num_shifts

candidate = candidate >> 16

if candidate < 1:
    return 1
if candidate > POW_LIMIT:
    return POW_LIMIT
return candidate
```

`trunc_div` means signed integer division truncated toward zero. Right shifts of signed values MUST have arithmetic semantics. Intermediate arithmetic MUST be wide enough to detect or avoid overflow; implementations may use arbitrary precision internally so long as the exact output is identical.

Because Oregon stores full targets, the final compact-target conversion steps from Bitcoin-derived ASERT implementations do not exist.

Golden vectors MUST cover negative/positive exponents, half-life boundaries, target clamps, large shifts, and implementation parity.

### 6.5 No emergency mainnet reset

Oregon mainnet v1 SHALL NOT contain a rule such as "if no block appears for N minutes, set difficulty to minimum."

Test/dev profiles may use separately defined non-mainnet chain parameters, but a minimum-difficulty escape path MUST NOT be silently active on mainnet.

## 7. Timestamp rules

### 7.1 Median time past

For candidate block `B` with parent `P`, define `MTP(P)` as follows:

1. Take timestamps of up to 11 blocks ending with `P` and walking backward through ancestors.
2. Sort ascending.
3. Select element `floor(count / 2)` using zero-based indexing.

This is the upper median for an even count during the first ten heights.

A non-genesis candidate is consensus-valid only if:

```text
timestamp(B) > MTP(P)
```

### 7.2 Future-time admission

A node SHOULD defer a block whose timestamp is more than one hour ahead of the node's local wall clock and reconsider it when time catches up.

The one-hour future bound is node admission policy, not a permanent consensus-invalidity rule. Local clock disagreement therefore cannot permanently fork consensus.

A candidate block's timestamp cannot alter its own target because the target is computed entirely from its parent chain. The timestamp can influence the next block's ASERT calculation.

## 8. Chain work and fork choice

Every valid non-genesis block contributes:

```text
block_work = floor(2^256 / (target + 1))
```

The work calculation MUST use arithmetic capable of representing `2^256` and `target + 1` without 256-bit wraparound; a 257-bit or arbitrary-precision intermediate is sufficient.

Genesis contributes zero chain work.

The active chain SHALL be the valid chain with greatest cumulative work, not greatest height.

For equal cumulative work, a node SHOULD retain its current active tip rather than repeatedly reorg on an arbitrary hash tie-breaker. Equal-work ties are temporary policy states; the next valid work normally resolves them.

## 9. Monetary issuance

### 9.1 Genesis

Genesis creates exactly `0 OREG`.

### 9.2 Mining subsidy

For height `h >= 1`:

```text
era = floor((h - 1) / 200000)
subsidy(h) = 237500000 >> era
```

All values are base units.

When the shifted value reaches zero, no further block subsidy exists. There are 28 positive-subsidy eras (`era 0` through `era 27`).

Exact scheduled mining issuance is:

`94,999,997,000,000` base units = `949,999.97000000 OREG`.

Adding the one-time founder allocation yields:

`99,999,997,000,000` base units = `999,999.97000000 OREG`.

The remaining `0.03000000 OREG` under the one-million envelope is intentionally unreachable. There is no final top-up mint.

### 9.3 Fees

For every normal transaction:

```text
fee = sum(input values) - sum(output values)
```

`sum(outputs)` MUST NOT exceed `sum(inputs)`.

Fees are not burned by protocol. A miner may claim up to the subsidy plus valid transaction fees. A miner may voluntarily under-claim; under-claiming permanently reduces realized supply.

There is no founder tax, treasury tax, developer percentage, protocol fee burn, admin mint, emergency mint, or continuing founder entitlement.

## 10. Coinbase consensus

### 10.1 Coinbase identity

Every non-genesis block MUST contain exactly one coinbase transaction and it MUST be transaction index `0`.

Coinbase version MUST be `1`, `lock_time` MUST be `0`, and canonical coinbase size MUST be at most 100 KiB.

It MUST contain exactly one input with:

- `previous_txid = 32 zero bytes`
- `previous_output_index = 0xffffffff`
- `sequence = 0xffffffff`

The first witness item MUST equal the exact canonical Oregon varint byte encoding of the current block height.

Additional witness items MAY carry miner extra-nonce or commitments within existing primitive and 100 KiB transaction limits.

No normal transaction may use the null outpoint.

### 10.2 Height 1 founder allocation

At height `1`, coinbase output index `0` MUST be exactly:

```text
value = 5,000,000,000,000 base units
locking_program = 0x01 || FOUNDER_KEY_COMMITMENT
```

This is the only protocol-authorized founder mint.

All coinbase outputs after index `0` are miner outputs. Their total MUST be less than or equal to:

```text
subsidy(1) + total_block_fees
```

The founder amount is outside that miner ceiling and is allowed only at height 1.

### 10.3 Heights greater than 1

For height `h > 1`, all coinbase output values combined MUST be less than or equal to:

```text
subsidy(h) + total_block_fees
```

No extra founder grant exists.

A miner is free to pay a normal reward to the founder's address, but it receives no special mint authority.

### 10.4 Coinbase maturity

Every output created by any coinbase, including the height-1 founder output, is spendable only if:

```text
spend_height >= creation_height + 120
```

This is ordinary coinbase maturity, not founder vesting.

## 11. UTXO state transition

### 11.1 UTXO entry

A chainstate UTXO entry SHALL contain at least:

```text
outpoint
value
locking_program
creation_height
is_coinbase
```

### 11.2 Normal transaction validity

A normal Oregon v1 transaction MUST satisfy all of the following:

- `version == 1`.
- It is not transaction index 0 of a block.
- It has at least one input.
- It has at least one output.
- Its canonical encoded size is at most 100 KiB.
- `lock_time == 0` in protocol v1.
- Every referenced outpoint exists in the current temporary UTXO view.
- No referenced outpoint is repeated within the transaction.
- No referenced outpoint is already spent earlier in the block.
- Coinbase maturity is satisfied when applicable.
- Every input authorization validates against its referenced output.
- Every output amount is within the monetary envelope.
- Checked input/output sums do not overflow.
- `sum(outputs) <= sum(inputs)`.

`sequence` is committed by signatures and transaction identity but has no lock/RBF consensus semantics in protocol v1. Any `u32` value is accepted for a normal input.

### 11.3 Block-local dependency order

A normal transaction may spend:

- an existing UTXO from an earlier block; or
- an output created by a transaction earlier in the same block.

A transaction MUST NOT reference an output created later in the same block. Thus block transaction order is a valid topological order of block-local dependencies.

### 11.4 Atomic connect/disconnect

Block validation SHALL operate against a temporary UTXO view.

No persistent chainstate mutation becomes visible until the entire block passes header, PoW, Merkle, transaction, signature, fee, and coinbase validation.

Connecting a block MUST produce undo information sufficient to restore every spent UTXO and remove every output created by the disconnected block.

Persistent UTXO/tip changes and undo indexing MUST commit atomically from the node's perspective.

## 12. Locking program and signature model

### 12.1 KeyCommitV1

Every output in a non-genesis v1 block, including coinbase outputs, MUST use the exact 33-byte KeyCommitV1 program:

```text
0x01 || key_commitment[32]
```

where:

```text
key_commitment = BLAKE3(
    "OREGON/KEY-COMMIT/V1\0" || x_only_secp256k1_pubkey[32]
)
```

Unknown locking-program versions are invalid for v1 chain consensus. There is no general script VM in v1. Value destruction is performed by under-claiming fees/subsidy or by voluntarily leaving input value unassigned to outputs, not by creating an unknown-script burn output.

### 12.2 Witness

To spend a KeyCommitV1 output, the corresponding input witness MUST contain exactly two items:

1. 32-byte x-only secp256k1 public key.
2. 64-byte BIP340 Schnorr signature.

The public key commitment MUST match the referenced output before signature verification is attempted.

### 12.3 Signature digest

Because Oregon transaction IDs include witness bytes, signatures MUST NOT sign the final txid.

The v1 signing digest is:

```text
BLAKE3(
    "OREGON/SIG/V1\0" ||
    chain_id[32] ||
    canonical_signing_payload
)
```

Binding `chain_id` prevents the same signed transaction from being replayed unchanged across Oregon networks with different genesis blocks.

`canonical_signing_payload` is encoded exactly as:

```text
transaction_version:u16
input_count:canonical_varint
for each input in transaction order:
    previous_txid:[u8;32]
    previous_output_index:u32
    sequence:u32
    referenced_value:u64
    referenced_locking_program_len:canonical_varint
    referenced_locking_program:bytes
output_count:canonical_varint
for each output in transaction order:
    value:u64
    locking_program_len:canonical_varint
    locking_program:bytes
lock_time:u64
signing_input_index:u32
```

No witness bytes are included.

Every v1 input uses this single all-input/all-output signing mode. There is no `SIGHASH_NONE`, `SIGHASH_SINGLE`, or `ANYONECANPAY` equivalent in v1.

The signature backend SHALL implement BIP340 verification semantics over secp256k1. The protocol is not tied to one Rust crate; at least one independent/backend parity test suite is required before mainnet.

## 13. Block validity order

A node SHOULD reject invalid data as cheaply as possible. Consensus result must be independent of validation order, but the first implementation SHOULD use this sequence:

1. Parse the fixed header and canonical block container within hard limits.
2. Verify parent existence/context.
3. Verify MTP rule.
4. Compute expected ASERT target and compare exact header target bytes.
5. Validate `1 <= target <= POW_LIMIT`.
6. Derive RandomX key and verify PoW.
7. Enforce 1 MiB canonical block size and 100 KiB per-transaction limits.
8. Require a non-empty transaction list and recompute transaction Merkle root.
9. Validate coinbase structural rules.
10. Validate normal transactions in order against a temporary UTXO view.
11. Verify KeyCommit/Schnorr authorizations.
12. Sum fees.
13. Validate coinbase founder/reward ceiling rules.
14. Commit UTXO, undo, block index, cumulative work, and active tip atomically.

A block with a valid PoW but invalid body MUST NOT affect chainstate.

## 14. Address encoding

Oregon v1 addresses use Bech32m checksums as an encoding layer, not Bitcoin witness-script semantics.

Decoded address payload bytes are exactly:

```text
address_version:u8 = 0x01
key_commitment:[u8;32]
```

For Bech32m data-part encoding, these 33 bytes are converted from 8-bit groups to 5-bit groups using zero padding. Decoders MUST reject non-zero padding, excess padding, mixed case, wrong checksum variant, wrong HRP for the selected network, unknown address versions, or payload lengths other than 33 bytes.

Mapping to consensus output is direct:

```text
address payload 0x01 || C
    -> locking_program 0x01 || C
```

HRPs:

- mainnet: `oreg`
- testnet: `toreg`
- dev/regtest: `doreg`

## 15. Network identity

For a chain profile with genesis block ID `G`:

```text
chain_id = BLAKE3("OREGON/CHAIN-ID/V1\0" || G)
p2p_magic = first_4_bytes(BLAKE3("OREGON/P2P-MAGIC/V1\0" || G))
```

P2P handshakes MUST bind both the genesis block ID and chain ID. A peer advertising a different chain MUST be disconnected before block synchronization.

## 16. Deterministic genesis and fair launch

### 16.1 Genesis block

Genesis is height `0` and is a hardcoded consensus anchor, not a RandomX-mined block.

Genesis header requirements:

- `version = 1`
- `previous_block = 32 zero bytes`
- `transaction_root = root of the single genesis transaction`
- `difficulty_commitment = INITIAL_TARGET`
- `nonce = 0`
- `timestamp = bitcoin_confirmation_mtp` from the manifest

Genesis creates no OREG.

### 16.2 Genesis transaction

Genesis contains exactly one special version-1 coinbase-like transaction with:

- one null-outpoint input using `sequence = 0xffffffff`
- `lock_time = 0`
- first witness item = canonical varint encoding of height `0`
- second witness item = canonical `GenesisManifestV1`
- zero outputs

The zero-output genesis transaction is a protocol exception. Normal transactions require outputs, and ordinary non-genesis coinbases follow Section 10.

### 16.3 GenesisManifestV1

The second witness item is encoded exactly as:

```text
manifest_magic = ASCII "OREGON-GENESIS-V1\0"
manifest_version:u16 = 1
bitcoin_anchor_height:u64
bitcoin_anchor_hash:[u8;32]
bitcoin_confirmation_height:u64
bitcoin_confirmation_hash:[u8;32]
bitcoin_confirmation_mtp:u64
source_commit_ascii:[u8;40]
consensus_spec_hash:[u8;32]
pow_limit:[u8;32]
initial_target:[u8;32]
founder_key_commitment:[u8;32]
```

Bitcoin hash fields are produced by decoding the standard lowercase 64-character Bitcoin block-hash display string left-to-right into 32 bytes.

`bitcoin_confirmation_mtp` is the median of the Bitcoin header timestamps at heights `H+1` through `H+11` inclusive, i.e. the confirmation block at `H+11` and its ten direct ancestors. Sort the 11 unsigned Unix timestamps ascending and select index `5`. This value is deterministic and reduces dependence on a single miner-selected Bitcoin timestamp.

`source_commit_ascii` is the exact lowercase 40-character Git commit hash of the frozen Oregon launch source.

`consensus_spec_hash` is:

```text
BLAKE3("OREGON/SPEC/V1\0" || exact_UTF8_bytes_of_frozen_consensus_spec)
```

`pow_limit` and `initial_target` are the exact 32-byte little-endian launch values and MUST match the frozen chain profile; genesis header `difficulty_commitment` MUST equal `initial_target`.

The manifest contains no private key or seed.

### 16.4 Public launch ceremony

The mainnet launch ceremony SHALL follow this order:

1. Freeze consensus source, reproducible-build instructions, exact RandomX vendoring/configuration, mainnet `POW_LIMIT`, `INITIAL_TARGET`, and founder public address/commitment.
2. Publish the frozen source commit and spec hash.
3. Announce a future Bitcoin anchor height `H` that is at least 1,008 Bitcoin blocks ahead of the Bitcoin tip observed at the announcement (approximately one week at target cadence).
4. No Oregon mainnet genesis exists before the announced Bitcoin anchor data exists.
5. When Bitcoin block `H` is mined, its exact hash is the anchor; no alternative block height/hash may be cherry-picked.
6. Wait until block `H` has 12 confirmations. The confirmation block is height `H + 11` on the same confirmed Bitcoin chain.
7. Compute `bitcoin_confirmation_mtp` from the confirmation block and its ten ancestors and use that MTP as Oregon genesis timestamp.
8. Deterministically generate the genesis transaction, Merkle root, block header, genesis block ID, chain ID, P2P magic, and address/network profile; verify the manifest `POW_LIMIT`, `INITIAL_TARGET`, and founder commitment against the frozen launch profile.
9. Publish genesis bytes/IDs, binary checksums, founder address, source hashes, and mining instructions together.
10. Public RandomX mining begins at Oregon height 1 under identical consensus rules for all miners.

If the frozen launch source/spec or any launch-profile consensus parameter must change after step 3 but before mainnet start, the ceremony MUST be aborted and restarted with a newly announced future Bitcoin anchor height. The previously announced anchor may not be silently reused for modified launch code.

This ceremony is a public time/commitment mechanism. It does not claim a mathematical proof that unauthorized private mining is impossible; height-1 acceptance by public nodes is the real consensus start.

## 17. First full-node architecture

The intended workspace expands toward:

```text
crates/
  oregon-primitives
  oregon-consensus
  oregon-crypto
  oregon-pow
  oregon-chain
  oregon-utxo
  oregon-storage
  oregon-mempool
  oregon-p2p
  oregon-sync
  oregon-mining
  oregon-rpc
node/
  oregon-node
```

Consensus-critical crates MUST have narrow interfaces and MUST NOT depend on RPC, wallet, or P2P policy layers.

### 17.1 P2P transport

Oregon's first public P2P transport is intended to be encrypted-only and to follow reviewed modern encrypted-transport principles, using BIP324 as a design reference rather than copying Bitcoin application messages.

There SHALL be no plaintext fallback in the production Oregon v1 network profile.

The transport MUST provide ciphertext integrity/authentication for its encrypted session. This requirement does not imply long-term peer identity authentication; Oregon v1 does not assume that a transport handshake proves who operates the remote node.

The encrypted-session application handshake SHALL include at least:

```text
protocol_version
chain_id
genesis_block_id
services
best_height
cumulative_work
session_nonce
```

The exact transport cryptographic transcript and wire-message schema require a dedicated P2P sub-spec before implementation. This master consensus spec fixes the encrypted-only/no-downgrade requirement and chain-binding requirement, not an improvised cryptographic protocol.

### 17.2 Headers-first synchronization

Initial synchronization SHOULD follow:

```text
peer discovery
-> encrypted handshake
-> headers
-> parent/time/target/PoW/work validation
-> best-work chain selection
-> block-body download
-> UTXO state transition
```

Peer queues, in-flight requests, bandwidth, and invalid-data processing MUST be bounded. Peer scoring is policy, not consensus.

Outbound peer selection SHOULD diversify network buckets to reduce eclipse/partition risk.

### 17.3 Storage

Consensus MUST depend on storage traits, not a database vendor.

Initial interfaces include:

- `BlockStore`
- `ChainIndex`
- `UtxoStore`
- `UndoStore`

A pure-Rust transactional backend such as `redb` is the preferred first implementation candidate, subject to implementation-time compatibility testing. Raw canonical blocks and undo records MAY use append-only segment files while indices/chainstate use transactional storage.

The first node is archival. Pruning and assumed UTXO snapshots are deferred.

### 17.4 Mempool policy

Mempool rules are explicitly non-consensus unless also stated elsewhere in this document.

The initial policy SHOULD support:

- conflict detection
- bounded orphan handling
- bounded total memory
- fee-rate eviction
- ancestor/descendant tracking
- full-RBF policy without a sequence opt-in flag
- ancestor-package-aware block selection for CPFP

Consensus permits zero-fee transactions; relay/mining policy may reject them.

Advanced package relay is deferred until measured need exists.

### 17.5 Mining RPC

Mining is external to the node process by default.

The first local authenticated JSON-RPC surface SHOULD expose equivalent operations to:

- `getminingtemplate`
- `submitblock`
- `getmininginfo`

A mining template MUST expose the parent, height, exact 256-bit target, valid time context, RandomX key/key-block context, subsidy ceiling, fees, required founder output at height 1, candidate transactions, and template identity.

`submitblock` MUST perform full independent consensus validation. It MUST NOT trust a miner's claimed target, fees, RandomX result, transaction validity, or founder-output construction.

RPC SHOULD bind to loopback by default and use local cookie/token authentication. Unauthenticated public admin/mining RPC is not a default mode.

A Stratum or decentralized pool gateway is a separate process/milestone, not consensus-node core.

## 18. Resource and policy boundaries

The foundation's 64 MiB decode limit is a hostile-object memory bound, not the block-size rule.

Protocol v1 block/transaction consensus sizes are:

- block: at most 1 MiB canonical bytes
- every transaction in a non-genesis block, including coinbase: at most 100 KiB canonical bytes

There is no witness discount or weight/vsize dual accounting in v1. A canonical byte counts as one byte.

## 19. Security posture

### 19.1 Adopt

- RandomX v2.0.1 family with Oregon-unique salt and pinned source.
- Miner-independent delayed key epochs.
- Per-block ASERT with 6-hour half-life.
- Full 256-bit target commitment.
- MTP-11 consensus time floor.
- Best cumulative-work fork choice.
- One-time transparent founder allocation.
- Integer-only monetary/accounting logic.
- Typed KeyCommitV1 payments with BIP340 Schnorr.
- Chain-ID-bound signature digests.
- Atomic UTXO/undo connect-disconnect.
- Encrypted-only public P2P architecture.
- Headers-first synchronization.
- Consensus/policy separation.

### 19.2 Experiment later

- P2Pool-like decentralized mining.
- Separate Stratum gateway.
- Tor/I2P transports.
- Compact block relay.
- More advanced package relay.
- Pruning.
- Verified UTXO snapshots.
- Additional locking-program versions after separate review.

### 19.3 Reject for v1

- DAG/blockDAG consensus.
- General smart-contract VM.
- Bitcoin Script compatibility.
- Account-state model.
- Miner-selected RandomX keys.
- Hardware/device miner attestation.
- Mainnet emergency minimum-difficulty reset.
- Continuing founder/developer tax.
- Treasury/admin mint.
- Hidden premine.
- Plaintext P2P fallback.
- Public unauthenticated admin RPC.
- Assume-valid consensus shortcuts in the first archival node.

## 20. Mainnet launch gates

Mainnet MUST NOT launch until all of the following are independently checkpointed:

1. RandomX v2.0.1 + Oregon salt produces identical Oregon vectors on x86-64 and ARM64, in light and full modes where supported.
2. ASERT has cross-implementation/golden vectors including negative exponents, clamps, large shifts, and long synthetic chains.
3. CPU benchmark data supports explicit `INITIAL_TARGET` and `POW_LIMIT` values without expected launch stall or uncontrolled burst.
4. Emission schedule tests prove exact `949,999.97 OREG` scheduled mining issuance and reject all over-claim mutations.
5. Founder height-1 output tests prove exact amount, exact index, exact commitment, and absence at later heights.
6. BIP340/KeyCommit signing vectors pass at least two independent implementations/backends.
7. UTXO connect/disconnect and deep synthetic reorg tests restore byte-for-byte equivalent chainstate.
8. Consensus validation mutation tests demonstrate that deliberately removing target, PoW, maturity, signature, double-spend, founder, subsidy, Merkle, and size checks causes the suite to fail.
9. Genesis/fair-launch generator is deterministic from the frozen manifest inputs, including Bitcoin MTP, target values, and founder commitment.
10. Reproducible release binaries/checksums and launch source are public before the future Bitcoin anchor.

If a consensus check is deliberately disabled and the relevant test suite remains green, the milestone is not accepted.

## 21. Implementation decomposition

This master design is intentionally larger than one coding milestone. Implementation SHALL be split so consensus can be reviewed before networking complexity is introduced.

Recommended sequence:

1. **M1 — Consensus Core:** `oregon-consensus`, emission, coinbase, target type, ASERT, chain work, block-context validation.
2. **M2 — PoW Engine:** `oregon-pow`, pinned RandomX v2.0.1 integration, Oregon configuration, key schedule, cross-mode vectors.
3. **M3 — Authorization + UTXO:** `oregon-crypto`, KeyCommitV1/BIP340, signing digest, UTXO state transition, maturity, undo.
4. **M4 — Chain + Storage:** active-chain selection, reorg engine, persistent block/UTXO/undo storage and crash recovery.
5. **M5 — Network Identity + Genesis Tools:** Bech32m addresses, chain profile, deterministic genesis/fair-launch generator.
6. **M6 — P2P + Sync:** dedicated encrypted transport spec, peer manager, headers-first synchronization.
7. **M7 — Mempool + Mining + RPC:** policy engine, block assembly, mining templates, submit path, local authenticated RPC.
8. **M8 — Public Testnet + Mainnet Readiness:** adversarial tests, benchmarks, parameter freeze, reproducible builds, launch ceremony.

Each milestone gets its own design/implementation checkpoint and may not silently revise this master consensus document.

## 22. Explicit non-claims

Approval of this design does not mean Oregon mainnet exists or that the implementation is production safe.

At this design stage there is no:

- numeric mainnet `POW_LIMIT` or `INITIAL_TARGET`;
- production founder private key in the repository;
- generated mainnet genesis;
- runnable public node;
- finalized P2P cryptographic wire transcript;
- wallet/key-management product;
- production pool or Stratum service.

These are gated later outputs, not implied features.

## 23. References used for design rationale

- RandomX upstream v2.0.1 and its PoW/configuration guidance.
- Bitcoin Cash ASERT `aserti3` fixed-point specification, adapted to Oregon's full-width target and 300-second/6-hour parameters.
- BIP340 Schnorr verification semantics.
- Bech32m checksum construction from BIP350.
- BIP324 encrypted-transport principles as a future Oregon P2P design reference.

References are inputs to review; Oregon consensus behavior is defined by this document and later frozen Oregon test vectors, not by silently following upstream behavior changes.