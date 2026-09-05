# Oregon Execution Envelope / Authorization Progress Checkpoint

Date: 2026-09-05

Status: **verified inactive execution primitive progress only**. This checkpoint is not activation approval and is not permission to integrate `main`.

## Source architecture and design

- Accepted execution architecture: `docs/superpowers/specs/2026-09-05-execution-architecture-design.md`.
- Byte-exact wire spec: `docs/superpowers/specs/2026-09-05-execution-envelope-wire-v1.md`.
- Implementation plan: `docs/superpowers/plans/2026-09-05-execution-envelope-wire-v1.md`.
- Design branch/source: `design/execution-envelope-wire-v1-2026-09-05` at `dd473f0277df1f82d51390332a5473a708031be0`, PR #11.
- Implementation branch: `work/execution-envelope-auth-v1-2026-09-05`, PR #12.
- Address dependency/checkpoint: `8057ba2a030a8e79c10a240d48675be758c4d875`.

## Delivered scope

`oregon-primitives` now owns an **inactive** V1 universal execution-envelope and typed authorization outer wire with:

- closed execution-domain tags: native `0x10`, EVM `0x11`, WASM `0x12`, system `0x13`;
- closed authorization scopes: principal `0x01`, fee payer `0x02`;
- closed authorization scheme ids: Oregon Schnorr V1 `0x0001`, Ethereum ECDSA source V1 `0x0002`, Oregon threshold/multi-proof V1 `0x0003`;
- canonical fixed-width little-endian fields plus Oregon's existing minimal varints;
- exact typed 33-byte execution addresses;
- hard structural bounds: envelope 2 MiB, domain payload 1 MiB, access hints 256 KiB, at most 2 authorizations, at most 4096 proof bytes;
- strict fee-payer/scope ordering rules;
- Ethereum source authorization restricted to the EVM domain and neutral Oregon height window `0..=u64::MAX`;
- native signing commitment domain `OREGON/ENVELOPE/SIGN/V1\0` excluding proof bytes while binding authorization scope/scheme and all authority-bearing common fields;
- full Oregon txid domain `OREGON/ENVELOPE/TXID/V1\0` over complete canonical bytes including proofs;
- fail-closed unknown version/domain/scope/scheme handling;
- bounded decode before copies/allocations, canonical option flags, truncation/trailing-byte rejection and non-minimal varint rejection at every variable-length position.

No signature cryptography is verified in this primitive layer; only outer structural validity is enforced.

## Test-first evidence

Expected-red source:

- commit `e7db4f4dc24193a9049d122311d1b6a0d16a5402`;
- Oregon Rust CI run `33977872761`;
- job `101337664670`;
- inherited execution-address contracts passed;
- envelope contract failed for the intended reason: `oregon_primitives::execution_envelope` did not yet exist (`E0432`).

Verified implementation source:

- commit `df62a75dfbde80dbea72e599ffbbccf0fb8fe1e0`;
- tree `47faba5451bc929804137f33de77b3973dabb0d2`;
- Oregon Rust CI run `33980111067`;
- job `101343713813`;
- conclusion: SUCCESS.

The exact verified source passed architecture scan, focused address contracts, focused envelope/golden/security/canonical contracts, full workspace/all-target tests, chainstate rustdoc, workspace docs, rustfmt and Clippy with warnings denied.

## Mutation evidence

Inherited execution-address gate:

- 3/3 mutations killed and restored suite passed.

Execution-envelope gate:

- 9/9 mutations killed and restored suite passed.
- Targets covered unknown-domain acceptance, identical explicit fee-payer acceptance, oversized-envelope bypass, missing fee-payer authorization, execution-domain omission from signing commitment, authorization-scope omission from signing commitment, proof inclusion in signing commitment, proof omission from txid, and mutable Ethereum source validity window.
- Mutation kills require the intended named test to fail; compilation failure alone is not accepted as a kill.

## Preserved boundaries

This stage does **not** activate the universal envelope. It does not modify or route through accepted `Transaction::encode/decode/txid`, block admission, mempool admission, chainstate, storage schema, RPC, wallet, network transaction relay, EVM execution, WASM execution, fee accounting or UTXO reserve movement.

The accepted M0–M6 transaction bytes and current native consensus path remain unchanged.

## Next stage

Execution Architecture V1 §27 Stage 2 is now the first incomplete item: **logical contract state and versioned commitments**.

Before Stage 2 implementation, write and approve a versioned design that freezes:

- authoritative state domains and ownership for EVM, WASM, execution OREG reserve/accounting and async-message state;
- deterministic state-root/commitment interfaces and domain separators;
- empty-state roots and canonical ordering/key/value commitments;
- versioned commitment envelope and upgrade/fail-closed behavior;
- extensible header-commitment boundary without prematurely mutating accepted header bytes;
- persistence, atomic publication, reorg/rollback and crash-recovery responsibilities;
- proof interfaces and explicit limits before allocation/verification;
- test vectors and mutation targets;
- one execution truth shared by later EVM and WASM backends.

Do not fill unspecified consensus details from Ethereum or another VM by convention. Main integration and any activation decision remain separate explicit gates.
