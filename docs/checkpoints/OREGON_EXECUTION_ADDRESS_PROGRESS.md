# Oregon Execution Address Progress Checkpoint

**Status:** verified implementation progress record; **not** activation acceptance and **not** permission to merge into `main`.

**Date:** 2026-09-05

## Authority and ancestry

- Accepted `main` for this work: `bf7675bfe17182f77d4c43e2bcbd0c283709d799`.
- Owner-approved Execution Architecture V1 source: `ed67ccb89131970571d93911cf5553be33636e2f`, branch `design/execution-architecture-v1-2026-09-05`, PR #9.
- Implementation branch: `work/execution-addresses-2026-09-05`, PR #10.
- Plan: `docs/superpowers/plans/2026-09-05-execution-addresses.md`.

The address work depends on the still-unmerged execution design PR. Neither this checkpoint nor PR #10 changes the requirement for a separate `main` integration decision.

## Delivered scope

The first part of Execution Architecture V1 §27 stage 1 is implemented as an **inactive primitive** in `oregon-primitives`:

- closed V1 namespace tags: EVM `0x01`, WASM `0x02`, Oregon execution identity `0x03`, protocol/system identity `0x04`;
- canonical internal identity width: exactly 33 bytes, `kind: u8 || payload: [u8; 32]`;
- EVM mapping: exactly twelve zero padding bytes followed by the original 20-byte Ethereum address;
- fail-closed rejection of unknown namespace tags, noncanonical EVM padding, truncation and trailing bytes;
- private validated fields so malformed addresses cannot be constructed through public decoding APIs;
- namespace-sensitive equality/hashing, preventing identical payload bytes from aliasing across domains;
- independently pinned literal vectors and property tests;
- a focused mutation gate for fail-open and namespace-assignment regressions.

The namespace tag is identity only. `System` does not confer protocol authority.

## Test-first evidence

The implementation followed a red/green sequence.

- Test/CI preparation head: `e7bae512cbf47f17b50d9db85c12dbca3ab874da`.
- Oregon Rust CI run: `33970929497`.
- Job: `101319185976`.
- Expected result: failure with Rust E0432 because `oregon_primitives::execution_address` did not yet exist.

This failure is the expected pre-implementation proof that the address contract was not already satisfied accidentally.

## Implementation and hardening

- Canonical address implementation commit: `445a0a92fd22409dd171bb915c2310203534ccca`.
- Independently pinned namespace-assignment hardening head: `52acaa1b645577314407372b4933d11eef5500cd`.
- Verified implementation tree: `e90a96247f8cc19bdd7340eb3e18dd9cbc349c1c`.

The extra hardening explicitly prevents a coordinated WASM/Oregon tag swap from escaping a mutation test merely because encoder and decoder changed together.

## Green verification

Exact head `52acaa1b645577314407372b4933d11eef5500cd` passed:

- Oregon Rust CI run `33971365738`: **SUCCESS**;
- Oregon RandomX Architecture Vector run `33971365737`: **SUCCESS**;
- Oregon RandomX Full Light Parity run `33971365761`: **SUCCESS**.

The Rust CI run includes:

- architecture scan;
- focused execution-address contracts;
- full workspace tests;
- execution-address mutation gates, **3/3 killed**;
- chainstate rustdoc with the repository's warnings policy;
- workspace docs;
- rustfmt;
- workspace Clippy.

The three deliberate address mutations are:

1. accept an unknown kind as a known namespace;
2. accept nonzero EVM left padding;
3. coherently reassign WASM and Oregon namespace wire tags.

Each mutation is killed by the intended named execution-address test rather than by an unrelated compilation failure.

## Preserved boundaries

This work does **not** change or activate:

- accepted M0–M6 transaction bytes, txids, native UTXO semantics, monetary policy or fee behavior;
- EVM or WASM execution;
- authorization/signature verification;
- universal-envelope serialization;
- mempool admission semantics;
- chainstate/state-root behavior;
- P2P protocol behavior;
- wallet/RPC behavior;
- human-readable address prefixes;
- bridge, privacy, DeFi, NFT, oracle or AI behavior.

No new dependency was required for the production primitive.

## Exact next action

Before any universal-envelope or authorization code is written, create a bounded inactive wire specification that freezes the consensus-facing details Execution Architecture V1 intentionally leaves open. At minimum it must define:

- exact field widths and discriminants;
- canonical field order;
- optional-field encoding and canonical absence/presence rules;
- authorization descriptor scheme identifiers and canonical descriptor layout;
- hard count and byte limits before expensive verification;
- domain payload and access-hint length encoding/limits;
- native Oregon signing commitment bytes and domain separation;
- exact relationship between normalized Ethereum ingress and the original signed Ethereum source bytes/hash;
- malformed/unknown version and unknown scheme fail-closed behavior;
- decoding/allocation rules suitable for adversarial input;
- vectors and deliberate mutation targets before implementation.

Do not infer these values from Ethereum conventions merely for compatibility. Ethereum ingress must normalize into the one authoritative Oregon execution truth without silently creating a second Oregon transaction format or authority path.

## Resume rule

Read root `HANDOFF.md` first. Continue from the latest descendant head of `work/execution-addresses-2026-09-05` unless a later handoff explicitly names a successor branch. Do not repeat this completed address primitive. Every descendant head must pass its own required CI before that exact head is described as verified.
