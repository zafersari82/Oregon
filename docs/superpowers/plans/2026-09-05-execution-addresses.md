# Execution Address Foundation Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Implement the inactive typed internal execution-address format already selected in Execution Architecture V1 §6 and preserve a discoverable cross-session handoff.

**Architecture:** `oregon-primitives` owns the exact 33-byte `kind || payload` representation. A private-field address type validates the four reserved kinds and canonical EVM padding at construction, so callers cannot manufacture a malformed address. No transaction, signing, authorization, VM, wallet, or activation path consumes this type yet.

**Tech Stack:** Rust 1.85.0, edition 2024, existing thiserror/proptest/serde test dependencies; no new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-05-execution-architecture-design.md` §§3.1, 6, 23–24, 27. This implements only the address portion of stage 1.

## Global constraints

- Starting design head: `ed67ccb89131970571d93911cf5553be33636e2f`; accepted main: `bf7675bfe17182f77d4c43e2bcbd0c283709d799`.
- Work branch: `work/execution-addresses-2026-09-05`; keep `main` and historical checkpoints unchanged.
- Address kinds: EVM `0x01`, WASM `0x02`, Oregon execution identity `0x03`, protocol/system identity `0x04`.
- Fixed representation: 1 kind byte followed by 32 payload bytes. EVM is 12 zero bytes followed by the original 20 bytes.
- Unknown kinds, nonzero EVM padding, truncation and trailing bytes fail closed; fixed arrays require no input-sized heap allocation.
- Kind is an identity namespace, not an authorization grant. Recognizing a system identity must not grant its holder authority.
- Preserve all M0–M6 bytes, vectors, monetary/UTXO rules, unsafe-code and dependency boundaries.
- Human-readable prefixes, Schnorr/ECDSA verification, envelope wire assignments and activation are outside this address-only plan.

## Task 1: Canonical typed address

**Files:**
- Create `crates/oregon-primitives/src/execution_address.rs`.
- Modify `crates/oregon-primitives/src/lib.rs` to expose the module only.
- Create `crates/oregon-primitives/tests/execution_addresses.rs`.
- Create `tests/vectors/execution-address-v1.json` with independently specified byte vectors.
- Create `scripts/verify_execution_address_mutations.py`; extend `.github/workflows/oregon-rust.yml` with the focused contract test before the full workspace gate and mutation checks afterward.

**Interfaces:**
- `ExecutionAddressKind::{Evm, Wasm, Oregon, System}` and `TryFrom<u8>`.
- `ExecutionAddress::new(kind: ExecutionAddressKind, payload: [u8; 32]) -> Result<Self, ExecutionAddressError>`.
- `ExecutionAddress::from_evm(address: [u8; 20]) -> Self`.
- `ExecutionAddress::from_slice(bytes: &[u8]) -> Result<Self, ExecutionAddressError>`.
- `ExecutionAddress::{kind, payload, to_bytes, evm_address}` expose validated data; `evm_address() -> Option<[u8; 20]>` returns `None` outside EVM.
- `ExecutionAddressError::{InvalidLength(usize), UnknownKind(u8), NonCanonicalEvmPadding}`.

- [ ] Write literal vectors for four namespaces, EVM zero/nonuniform/all-ones addresses, and malformed encodings. Add exhaustive unknown-kind rejection, every truncated length, oversized input, every EVM padding byte, namespace-identity and property tests. Catch tag collapse, lost payload bytes, truncating decoders and EVM padding aliases.
- [ ] Run the tests before implementation. Example assertion:

```rust
assert_eq!(
    ExecutionAddress::from_slice(&[0u8; 33]),
    Err(ExecutionAddressError::UnknownKind(0))
);
```

Run `cargo +1.85.0 test --locked -p oregon-primitives --test execution_addresses` and observe missing module/API. If the local toolchain is absent, push the test-only commit and use the existing GitHub Rust CI; missing local Cargo is not a test result.

- [ ] Implement the specified API using private fields, exact slice-to-array conversion and one constructor validation owner. `from_slice` delegates to `new`; EVM extraction checks the stored kind. Do not silently normalize malformed input.
- [ ] Run the focused test and inherited workspace/format/Clippy/docs/architecture gates. Preserve exact commit/run IDs.
- [ ] Check two deliberate security mutations: accepting unknown kinds and allowing nonzero EVM padding. Each must fail a named address rejection test, not merely fail compilation.
- [ ] Commit and publish the clean implementation branch with verified source-tree identity.

## Task 2: Discoverable continuation record

**Files:** create root `HANDOFF.md`, link it from `AGENTS.md` and `README.md`, and create `docs/checkpoints/OREGON_EXECUTION_ADDRESS_PROGRESS.md` as an implementation progress record, not a mainnet/activation acceptance.

- [ ] Record accepted main, source design branch/PR #9, active work branch/PR, plan path, test-only and clean implementation SHAs, CI evidence and limitations.
- [ ] State the exact next action: define the bounded inactive universal-envelope and authorization-descriptor wire specification before implementing it. §4 currently specifies logical fields but does not freeze all widths, discriminants, canonical options/order, limit constants or signing/source-byte commitments.
- [ ] Preserve the §27 stage sequence. Do not invent missing envelope rules from Ethereum conventions or repeat already completed M6 work.
- [ ] Record the resume procedure: fetch current refs, inspect this handoff at the active branch, read normative docs, compare working tree to origin, and continue the first incomplete item. Commit/push after coherent verified increments; never label local-only work as remotely saved.
- [ ] Verify every changed file exists at the remote branch and local/remote Git tree hashes match. Keep a draft PR available for discovery; integration requires its own owner decision.

## Scope review

This plan implements the exact approved address representation without assigning previously unspecified consensus parameters. Other execution primitives, full envelope/authorization and stages 2–10 remain open; the address work is not execution-milestone acceptance.
