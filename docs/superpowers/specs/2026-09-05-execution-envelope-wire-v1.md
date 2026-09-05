# Oregon Universal Envelope and Authorization Wire V1

**Status:** implementation specification for the already owner-approved Execution Architecture V1; inactive until a later explicit activation checkpoint

**Date:** 2026-09-05

**Parent architecture:** `docs/superpowers/specs/2026-09-05-execution-architecture-design.md`

**Source checkpoint:** `8057ba2a030a8e79c10a240d48675be758c4d875`

## 1. Purpose

This document makes Execution Architecture V1 §4, §7, §8, §23, §24 and §27 stage 1 byte-exact enough to implement the inactive universal-envelope and authorization primitives. It does not alter the already approved architecture and it does not activate a new transaction format.

The accepted M0–M6 `Transaction` encoding, txid rules, UTXO semantics, mempool behavior and block commitments remain unchanged. No existing decoder accepts this envelope until a later activation design explicitly wires it into consensus.

## 2. Existing canonical encoding rules reused

The envelope reuses `oregon-primitives` canonical rules rather than creating a parallel codec:

- fixed-width integers are little-endian;
- variable lengths/counts use the existing minimal `write_varint` / `Decoder::read_varint` encoding;
- non-minimal varints fail closed;
- every length is checked against a hard limit before copying or allocating attacker-controlled bytes;
- truncation fails closed;
- trailing bytes fail closed;
- unsupported versions, domains, scopes and authorization schemes fail closed.

No TLV skip-unknown behavior exists in V1.

## 3. Hard structural limits

These are parser/resource ceilings for the inactive V1 wire object, not the later benchmark-backed execution/block weight limits:

- `MAX_ENVELOPE_BYTES = 2_097_152` (2 MiB)
- `MAX_DOMAIN_PAYLOAD_BYTES = 1_048_576` (1 MiB)
- `MAX_ACCESS_HINT_BYTES = 262_144` (256 KiB)
- `MAX_AUTH_PROOFS = 2`
- `MAX_AUTH_PROOF_BYTES = 4_096`

A later activation may impose lower consensus/mempool limits. It may not raise these V1 wire ceilings without a versioned format change.

## 4. V1 discriminants

### 4.1 Envelope version

`envelope_version` is `u16` little-endian. V1 is exactly `1`. Any other value is unsupported and fails closed.

### 4.2 Execution domain

`execution_domain` is one `u8`:

- `0x10` — Oregon native/UTXO execution domain
- `0x11` — EVM execution domain
- `0x12` — WASM execution domain
- `0x13` — protocol/system execution domain

`0x00` and all other values are invalid in V1. These tags are deliberately a separate namespace from `ExecutionAddressKind`; numeric coincidence must never be used to convert one enum into the other.

### 4.3 Authorization scope

`scope` is one `u8`:

- `0x01` — principal authorization
- `0x02` — distinct fee-payer authorization

No other scope exists in V1.

### 4.4 Authorization scheme

`scheme_id` is `u16` little-endian:

- `0x0001` — Oregon-native secp256k1 Schnorr V1
- `0x0002` — Ethereum-compatible secp256k1 ECDSA source authorization V1
- `0x0003` — Oregon bounded threshold/multi-proof V1

Unknown scheme ids fail closed. The threshold scheme is a recognized V1 descriptor namespace, but cryptographic threshold identity/member semantics remain owned by the later authorization verifier; the stage-1 wire primitive only enforces the canonical outer framing and byte limit.

## 5. Canonical envelope byte order

A V1 envelope is encoded in exactly this order:

1. `envelope_version: u16`
2. `chain_id: u64`
3. `execution_domain: u8`
4. `valid_after_height: u64`
5. `valid_until_height: u64`
6. `principal: [u8; 33]` — canonical `ExecutionAddress`
7. `fee_payer_present: u8`
8. if present, `fee_payer: [u8; 33]`
9. `max_fee_per_weight: u64`
10. `max_priority_fee_per_weight: u64`
11. `max_weight: u64`
12. `authorization_count: canonical varint`
13. each authorization proof in canonical scope order
14. `domain_payload_len: canonical varint`
15. `domain_payload: [u8; domain_payload_len]`
16. `access_hints_present: u8`
17. if present, `access_hints_len: canonical varint`
18. if present, `access_hints: [u8; access_hints_len]`

No field may be omitted except through the explicit one-byte optional encodings below.

## 6. Canonical optional values

### 6.1 Fee payer

`fee_payer_present` is exactly `0x00` or `0x01`.

- `0x00`: fee payer is semantically the principal and no fee-payer bytes follow.
- `0x01`: one canonical 33-byte execution address follows.

A present fee payer equal to `principal` is rejected as noncanonical. This prevents two byte encodings for the same semantic payer.

### 6.2 Access hints

`access_hints_present` is exactly `0x00` or `0x01`.

- `0x00`: no hint bytes follow.
- `0x01`: a canonical varint length and a non-empty hint blob follow.

Present-but-empty hints are rejected as noncanonical. The hint blob is domain-owned optimization metadata; it is signed and txid-committed but cannot weaken authoritative validity if absent.

## 7. Height window

Both heights are `u64`.

- `valid_after_height = 0` means no lower restriction before genesis-relative height zero.
- `valid_until_height = u64::MAX` is the canonical unbounded upper value.
- `valid_after_height > valid_until_height` is structurally invalid.

Ethereum normalization uses the protocol-fixed neutral pair `0` and `u64::MAX`, because a standard Ethereum signer did not authorize Oregon-specific mutable height expiry.

## 8. Fee-cap structural rules

The three fee/resource fields are `u64`:

- `max_fee_per_weight`
- `max_priority_fee_per_weight`
- `max_weight`

Structural canonical rules:

- `max_weight` must be nonzero;
- `max_priority_fee_per_weight <= max_fee_per_weight`;
- overflow-sensitive fee arithmetic is not performed in the primitive decoder and later execution uses checked/wider arithmetic.

Whether a fee cap is sufficient for a specific block is execution/consensus validity, not primitive decoding.

## 9. Authorization proof framing and order

Each authorization proof is encoded:

1. `scope: u8`
2. `scheme_id: u16`
3. `proof_len: canonical varint`
4. `proof_bytes`

Outer framing rules:

- proof bytes are non-empty;
- `proof_len <= MAX_AUTH_PROOF_BYTES` before copying;
- authorizations are strictly ordered by scope: principal first, fee payer second;
- duplicate scopes are invalid;
- exactly one principal authorization is required;
- if `fee_payer_present == 0`, a fee-payer authorization is forbidden;
- if `fee_payer_present == 1`, exactly one fee-payer authorization is required;
- therefore authorization count is exactly one or two in V1;
- Ethereum ECDSA source authorization is valid only in the EVM execution domain at the structural scheme/domain boundary.

Threshold/multi-proof remains one authorization object for one scope. Its bounded internal proof body does not create extra top-level authorization entries.

## 10. Scheme proof outer lengths

Stage-1 structural checks additionally pin:

- Oregon Schnorr V1: exactly 96 proof bytes (`32-byte public key || 64-byte signature`);
- Ethereum ECDSA V1: exactly 65 proof bytes (canonical recoverable signature material used by the later Ethereum normalization verifier);
- Oregon threshold/multi-proof V1: `1..=4096` bytes; its internal canonical member/threshold semantics are validated by the authorization subsystem before activation and are not guessed by the generic envelope decoder.

The generic wire decoder does not perform signature verification.

## 11. Domain payload

`domain_payload` is a canonical length-prefixed byte string up to `MAX_DOMAIN_PAYLOAD_BYTES`. Zero length is permitted because a domain may define a valid empty operation in a later versioned payload spec.

The envelope decoder treats payload bytes as opaque. Only the authoritative selected execution domain interprets them.

Ethereum-compatible normalization must later define a versioned EVM payload codec that commits to the exact signed Ethereum source transaction hash and the canonical normalized semantic fields. The adapter may not discard source identity, invent unsigned authority-bearing fields, or produce a second authoritative tx identity.

## 12. Native Oregon signing commitment

Native Oregon authorization schemes (`0x0001` and `0x0003`) sign one common commitment so principal and distinct fee payer authorize the same transaction intent.

Signing bytes are the canonical envelope fields in the same order as §5, except each authorization entry contributes only:

- `scope`
- `scheme_id`

and **does not include `proof_len` or `proof_bytes`**.

The authorization count and descriptor order remain committed. Every other common field — including chain id, execution domain, validity window, both identities, fee caps, domain payload and access hints — is included.

Signing hash:

`BLAKE3("OREGON/ENVELOPE/SIGN/V1\0" || signing_bytes)`

This prevents proof-byte circularity while preventing substitution of chain, domain, payer, fee cap, payload, access hints or authorization scheme after signing.

Ethereum ECDSA authorization does not sign this Oregon-native commitment. It verifies the canonical signed Ethereum source according to the later EVM normalization spec, while the normalized envelope txid still commits to the resulting complete Oregon envelope.

## 13. Canonical Oregon envelope txid

The internal Oregon transaction identity for an activated universal envelope is:

`BLAKE3("OREGON/ENVELOPE/TXID/V1\0" || canonical_full_envelope_bytes)`

The full bytes include authorization proof bytes. Changing a proof therefore changes the Oregon txid even when the unsigned intent is identical.

An Ethereum transaction hash remains a compatibility alias/index only. It never replaces this Oregon txid.

## 14. Decode and allocation order

The decoder must:

1. reject input longer than `MAX_ENVELOPE_BYTES` before decoding;
2. decode fixed fields without allocation;
3. validate each discriminant immediately;
4. validate fee-payer/address canonicality before proceeding;
5. read authorization count under `MAX_AUTH_PROOFS`;
6. read and bound each proof length before copying proof bytes;
7. bound domain payload length before copying;
8. bound access-hint length before copying;
9. run structural cross-field canonicality checks; and
10. require exact end-of-input.

No attacker-declared length is used for unchecked preallocation.

## 15. Required stage-1 vectors and mutations

Before implementation is called verified, tests must independently pin:

- every version/domain/scope/scheme discriminant;
- exact minimum envelope bytes;
- principal and distinct fee-payer encodings;
- canonical absent/present option encodings;
- neutral Ethereum height window;
- fee-cap boundary rejection;
- minimal-varint boundaries and non-minimal rejection inherited through the envelope;
- every truncation boundary for the minimum envelope;
- trailing-byte rejection;
- payload/hint/proof exact limits and one-byte-over limits;
- canonical authorization order and duplicate-scope rejection;
- signing-hash sensitivity to every signed field while remaining independent of proof bytes;
- full txid sensitivity to proof bytes;
- cross-chain and cross-domain signing commitment separation.

Required deliberate mutations include at least:

1. unknown domain accepted as EVM;
2. present fee payer equal to principal accepted;
3. non-minimal or over-limit length accepted;
4. fee-payer authorization omitted or duplicate scope accepted;
5. domain omitted from native signing commitment;
6. proof bytes incorrectly included in the native signing commitment;
7. proof bytes omitted from the full txid;
8. Ethereum neutral height window made mutable by the adapter contract.

Each mutation must be killed by the intended named contract test, not by unrelated compilation failure.

## 16. Non-activation guarantee

Implementing this specification only adds inactive primitives and tests. It must not:

- change accepted `Transaction::encode/decode/txid`;
- make blocks accept envelope bytes;
- change mempool admission;
- activate fee-market behavior;
- verify production spend authorization;
- add EVM/WASM execution;
- add RPC/wallet behavior; or
- alter M0–M6 consensus or persistence semantics.

Activation remains a later explicit versioned migration and checkpoint decision under the Engineering Constitution.
