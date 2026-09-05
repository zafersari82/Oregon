# Universal Envelope and Authorization Wire V1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Status:** COMPLETE and verified on implementation source `df62a75dfbde80dbea72e599ffbbccf0fb8fe1e0` (tree `47faba5451bc929804137f33de77b3973dabb0d2`).

**Goal:** Implement the already-approved universal Oregon envelope and typed authorization outer wire as inactive canonical primitives without changing accepted M0–M6 transaction behavior.

**Architecture:** `oregon-primitives` owns one byte-exact V1 envelope codec using the existing canonical little-endian/minimal-varint decoder. The type validates structural canonicality, exposes Oregon-native signing and full txid commitments, and leaves domain payload execution/signature verification to later authoritative subsystems. No existing `Transaction`, block, mempool, chainstate, RPC or VM path consumes this type.

**Spec:** `docs/superpowers/specs/2026-09-05-execution-envelope-wire-v1.md`

## Global Constraints

- Base/source checkpoint: `8057ba2a030a8e79c10a240d48675be758c4d875`.
- Design branch/source: `design/execution-envelope-wire-v1-2026-09-05` / `dd473f0277df1f82d51390332a5473a708031be0`.
- Preserve `Transaction::encode/decode/txid` and all M0–M6 bytes exactly.
- Fixed-width little-endian integers + canonical minimal varints; no TLV skip-unknown behavior.
- Hard ceilings: envelope 2 MiB, payload 1 MiB, access hints 256 KiB, at most 2 authorizations, at most 4096 proof bytes.
- Domains `0x10..0x13`, scopes `0x01..0x02`, schemes `0x0001..0x0003` frozen by tests/vectors.
- Native signing domain `OREGON/ENVELOPE/SIGN/V1\0`.
- Full txid domain `OREGON/ENVELOPE/TXID/V1\0`.
- Ethereum neutral validity window exactly `0..=u64::MAX`.
- Unknown versions/domains/scopes/schemes and malformed/noncanonical encodings fail closed.
- No production unsafe code and no new dependencies.

## Task 1 — Test-first canonical wire contract

- [x] Independent literal canonical vectors for native, distinct-fee-payer and neutral-window EVM forms.
- [x] Frozen domain/scope/scheme discriminants and canonical option tests.
- [x] Structural boundary tests for validity window, fee caps, proof count/length/scope, Ethereum authorization domain/window and byte limits.
- [x] Adversarial codec tests for non-minimal varints at every variable-length position, truncation, trailing bytes and total-size rejection.
- [x] Commitment tests for every authority-bearing common field; proof bytes excluded from native signing commitment and included in txid.
- [x] Expected-red checkpoint published at `e7db4f4dc24193a9049d122311d1b6a0d16a5402`, run `33977872761`, job `101337664670`, failing as intended with missing envelope module/API.

## Task 2 — Inactive envelope and authorization primitives

- [x] Closed discriminant enums with fail-closed `TryFrom`.
- [x] Private-field `AuthorizationProof` with scheme-specific outer lengths and read-only getters; no signature verification here.
- [x] `FeeCaps` rejecting zero weight and priority fee above max fee.
- [x] Validated envelope construction with height, fee-payer, authorization order/scope, Ethereum-domain and payload/hint rules.
- [x] Canonical encoding using existing `write_varint`.
- [x] Bounded decoding with total-size-first rejection, existing `Decoder`, 33-byte address decoder and trailing-byte rejection.
- [x] Native signing bytes/hash and full txid commitments implemented with frozen domain separators.
- [x] Focused and full green verification completed.

## Task 3 — Security mutation gates and persistent checkpoint

- [x] Focused mutation runner uses disposable worktree/restore behavior and requires intended named test failure rather than compilation failure.
- [x] At least eight mutants required; implementation kills **9/9**, including an additional authorization-scope signing-commitment mutant.
- [x] CI order runs focused envelope contracts before full workspace and mutation gates after green full tests while preserving inherited address/M0–M6 gates.
- [x] Red/green/mutation evidence recorded in `docs/checkpoints/OREGON_EXECUTION_ENVELOPE_PROGRESS.md`.
- [x] `HANDOFF.md` marks typed address and envelope/auth outer wire complete and points to Stage 2.
- [x] Remote source identity verified: `df62a75dfbde80dbea72e599ffbbccf0fb8fe1e0`, tree `47faba5451bc929804137f33de77b3973dabb0d2`, Rust CI `33980111067` / job `101343713813` SUCCESS.

## Verification summary

- Architecture scan: SUCCESS.
- Execution address contracts: SUCCESS.
- Execution envelope contracts: SUCCESS.
- Full workspace/all-targets: SUCCESS.
- Address mutations: 3/3 killed.
- Envelope mutations: 9/9 killed.
- Chainstate rustdoc: SUCCESS.
- Workspace docs: SUCCESS.
- rustfmt: SUCCESS.
- Clippy `-D warnings`: SUCCESS.

## Non-activation boundary

This completed plan delivered inactive primitives only. `crates/oregon-primitives/src/transaction.rs` and accepted transaction/block/mempool/chainstate/RPC/wallet/VM activation paths were not changed by this stage.

## Next plan

Do not add more envelope/auth outer-wire code. The next architectural task is Execution Architecture V1 §27 Stage 2: **logical contract state and versioned commitments**. Write its versioned design and implementation plan before code.
