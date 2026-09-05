# Universal Envelope and Authorization Wire V1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the already-approved universal Oregon envelope and typed authorization outer wire as inactive canonical primitives without changing accepted M0–M6 transaction behavior.

**Architecture:** `oregon-primitives` owns one byte-exact V1 envelope codec using the existing canonical little-endian/minimal-varint decoder. The new type validates structural canonicality, exposes Oregon-native signing and full txid commitments, and leaves domain payload execution/signature verification to later authoritative subsystems. No existing `Transaction`, block, mempool, chainstate, RPC or VM path consumes this type.

**Tech Stack:** Rust 1.85.0, edition 2024, existing `thiserror`, `serde`, `serde_json`, `proptest`, BLAKE3 and current `oregon-primitives` codec.

**Spec:** `docs/superpowers/specs/2026-09-05-execution-envelope-wire-v1.md`

## Global Constraints

- Base/source checkpoint: `8057ba2a030a8e79c10a240d48675be758c4d875`.
- Design branch: `design/execution-envelope-wire-v1-2026-09-05`.
- Preserve `Transaction::encode/decode/txid` and all M0–M6 bytes exactly.
- Reuse fixed-width little-endian integers and canonical minimal varints; no TLV skip-unknown behavior.
- Hard ceilings: envelope 2 MiB, payload 1 MiB, access hints 256 KiB, at most 2 authorizations, at most 4096 proof bytes.
- Domains: `0x10` native, `0x11` EVM, `0x12` WASM, `0x13` protocol/system.
- Scopes: `0x01` principal, `0x02` fee payer.
- Schemes: `0x0001` Oregon Schnorr, `0x0002` Ethereum ECDSA source, `0x0003` bounded threshold/multi-proof.
- Native signing domain: `OREGON/ENVELOPE/SIGN/V1\0`.
- Full txid domain: `OREGON/ENVELOPE/TXID/V1\0`.
- Ethereum neutral validity window is exactly `0..=u64::MAX`.
- Unknown versions/domains/scopes/schemes and malformed/noncanonical encodings fail closed.
- No production unsafe code and no new dependencies.

---

### Task 1: Test-first canonical wire contract

**Files:**
- Create: `crates/oregon-primitives/tests/execution_envelopes.rs`
- Create: `tests/vectors/execution-envelope-v1.json`
- Modify: `.github/workflows/oregon-rust.yml`

**Interfaces consumed:**
- `ExecutionAddress` from `oregon_primitives::execution_address`.

**Interfaces expected from Task 2:**
- `ExecutionDomain::{Native,Evm,Wasm,System}`
- `AuthorizationScope::{Principal,FeePayer}`
- `AuthorizationScheme::{OregonSchnorrV1,EthereumEcdsaV1,OregonThresholdV1}`
- `AuthorizationProof::new(scope, scheme, proof: Vec<u8>) -> Result<Self, ExecutionEnvelopeError>`
- `FeeCaps::new(max_fee_per_weight, max_priority_fee_per_weight, max_weight) -> Result<Self, ExecutionEnvelopeError>`
- `ExecutionEnvelopeV1Parts { chain_id, execution_domain, valid_after_height, valid_until_height, principal, fee_payer, fee_caps, authorizations, domain_payload, access_hints }`
- `ExecutionEnvelopeV1::new(parts) -> Result<Self, ExecutionEnvelopeError>`
- `ExecutionEnvelopeV1::{encode, decode, signing_bytes, signing_hash, txid}`

- [ ] **Step 1: Add independent literal vectors.** Include at minimum: one native principal-only envelope, one distinct-fee-payer envelope, one EVM neutral-window envelope, payload/hints present and absent forms, and literal expected signing-hash/txid values. JSON stores canonical hex rather than deriving expected bytes from production encoding.

- [ ] **Step 2: Add discriminant and canonical-option tests.** Pin domain tags `10..13`, scope tags `01..02`, scheme ids `0001..0003`, option flags `00/01`, reject all unknown tags, reject fee payer equal to principal, reject present-empty access hints, and reject option flags other than 0/1.

- [ ] **Step 3: Add structural boundary tests.** Assert `valid_after <= valid_until`, EVM neutral window vector `0/u64::MAX`, `max_weight > 0`, priority fee not above max fee, exact proof lengths (Schnorr 96, ECDSA 65, threshold 1..=4096), exact proof count/scope ordering, missing/duplicate scope failures, Ethereum ECDSA outside EVM rejection, payload/hint/proof limits and +1 failures.

- [ ] **Step 4: Add codec adversarial tests.** Test non-minimal varints in every length/count position, every truncation boundary of a minimum envelope, extra trailing bytes, and total-envelope >2 MiB rejection before deeper decode.

- [ ] **Step 5: Add commitment tests.** For every signed field, mutate one value and assert `signing_hash` changes; mutate only proof bytes and assert `signing_hash` is unchanged but `txid` changes. Assert chain-id and domain changes alter native signing commitment.

- [ ] **Step 6: Publish expected-red checkpoint.** Run `cargo +1.85.0 test --locked -p oregon-primitives --test execution_envelopes`. The expected failure is missing `execution_envelope` module/API. If local Rust is unavailable, push only the tests/workflow changes and use GitHub Actions. Record exact commit/run/job ids.

---

### Task 2: Inactive envelope and authorization primitives

**Files:**
- Create: `crates/oregon-primitives/src/execution_envelope.rs`
- Modify: `crates/oregon-primitives/src/lib.rs`
- Do not modify: `crates/oregon-primitives/src/transaction.rs`

**Produces:** the interfaces named in Task 1.

- [ ] **Step 1: Implement closed discriminant enums.** `TryFrom` rejects every unsupported value. Keep execution-domain enum independent from `ExecutionAddressKind` even when callers hold both.

- [ ] **Step 2: Implement `AuthorizationProof`.** Private fields; validate non-empty proof, scheme-specific outer length, and expose read-only getters. Do not verify signatures here.

- [ ] **Step 3: Implement `FeeCaps`.** Reject zero `max_weight` and `max_priority_fee_per_weight > max_fee_per_weight`; expose read-only values.

- [ ] **Step 4: Implement validated envelope construction.** Private envelope fields; reject invalid height window, identical present fee payer, bad authorization count/order/scope requirements, Ethereum ECDSA outside EVM, over-limit payload/hints and empty present hints.

- [ ] **Step 5: Implement canonical encoding.** Encode fields exactly in spec §5 using existing `write_varint`. Authorizations encode `scope || scheme_id LE || proof_len || proof`.

- [ ] **Step 6: Implement bounded decoding.** Reject total length first; use existing `Decoder` reads; validate each count/length before copying; decode 33-byte addresses through `ExecutionAddress::from_slice`; require `finish()`.

- [ ] **Step 7: Implement commitments.** `signing_bytes` mirrors full encoding but replaces each authorization with only `scope || scheme_id`; `signing_hash` uses `domain_hash(b"OREGON/ENVELOPE/SIGN/V1\0", ...)`; `txid` uses `domain_hash(b"OREGON/ENVELOPE/TXID/V1\0", &encode())`.

- [ ] **Step 8: Run focused green.** Run `cargo +1.85.0 test --locked -p oregon-primitives --test execution_envelopes` and the crate tests. Then run full workspace tests, rustdoc/docs, rustfmt, Clippy and architecture scan.

---

### Task 3: Security mutation gates and persistent checkpoint

**Files:**
- Create: `scripts/verify_execution_envelope_mutations.py`
- Modify: `.github/workflows/oregon-rust.yml`
- Create: `docs/checkpoints/OREGON_EXECUTION_ENVELOPE_PROGRESS.md`
- Modify: `HANDOFF.md`

- [ ] **Step 1: Add focused mutation runner.** On a disposable checkout, require a clean baseline and apply one mutant at a time, restore source in `finally`, and require the intended exact named test to fail rather than accepting compilation failure as a kill.

- [ ] **Step 2: Kill at least eight mutants.** Cover: unknown-domain acceptance; identical fee-payer acceptance; over-limit/nonminimal length acceptance; missing fee-payer authorization; domain omitted from signing bytes; proof bytes included in signing bytes; proof bytes omitted from txid; and neutral EVM validity made mutable/incorrect.

- [ ] **Step 3: Wire CI order.** Run focused envelope contracts before full workspace tests and envelope mutation gates after a green full test step. Preserve existing address and inherited M0–M6 gates.

- [ ] **Step 4: Record red/green/mutation evidence.** Checkpoint must name source architecture/spec, branch/PR, expected-red SHA/run/job, clean implementation SHA/tree, full CI run, mutation result and non-activation limitations.

- [ ] **Step 5: Update `HANDOFF.md`.** Mark typed address and envelope/auth outer wire completed only after the exact descendant head passes its own CI. Name stage 1’s next incomplete item rather than repeating completed work.

- [ ] **Step 6: Verify remote identity.** Confirm the branch ref, exact commit/tree and required files from GitHub before reporting saved/verified status. Integration to `main` remains separate.

## Self-review

- Spec coverage: every wire field, discriminant, optional encoding, bound, commitment rule and malformed-input requirement has a corresponding task/test.
- No implementation touches current `transaction.rs` or activation paths.
- Type names/signatures are consistent across tasks.
- No placeholder/TODO implementation step remains.
