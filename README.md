# Oregon (OREG)

Oregon is an experimental, independent proof-of-work blockchain protocol implemented in Rust. The accepted development baseline currently covers the protocol foundation through the M6 networking and node-orchestration milestone (M0-M6).

The repository is a clean-room implementation. Bitcoin and earlier Radium materials were used only as historical research inputs; Oregon does not import or patch their implementation code.

## Implemented baseline

The accepted M6 baseline includes:

- canonical transaction and block encoding, identifiers, Merkle commitments, and decode limits;
- fixed monetary representation, founder allocation, mining emission, and halving rules;
- header validation, ASERT difficulty adjustment, targets, and cumulative chainwork;
- Oregon-bound RandomX proof of work with frozen key scheduling and full/light parity vectors;
- UTXO transitions, mandatory spend-verifier boundary, exact coinbase maturity, and undo data;
- RocksDB persistence with atomic batches, WAL, synchronous accepted-state commits, recovery checks, migration policy, and pruning;
- active-chain extension, strictly-heavier-work reorganization, bounded reorg policy, and fail-closed recovery;
- a deterministic policy mempool with one spender per outpoint, no RBF, no orphan pool, bounded dependency graphs, eviction, and atomic chain reconciliation;
- bounded protocol v1 framing and feature negotiation;
- bounded TCP transport, peer handshake/lifecycle, self/duplicate-peer rejection, request matching, liveness, scoring, cooldowns, and inventory knowledge tracking;
- headers-first, fork-aware synchronization with bounded per-peer/global block scheduling and timeout reassignment; and
- node orchestration that keeps ChainState and Mempool authoritative, validates before relay, slices remote header batches, and is exercised by real loopback TCP integration/resilience tests.

Accepted milestone records are under [`docs/checkpoints`](docs/checkpoints). The current engineering contract is [`docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md`](docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md). Owner-approved future platform direction is frozen in [`docs/architecture/OREGON_PLATFORM_ARCHITECTURE_CONTRACT.md`](docs/architecture/OREGON_PLATFORM_ARCHITECTURE_CONTRACT.md), with the detailed execution/state/fee/VM architecture frozen in [`docs/superpowers/specs/2026-09-05-execution-architecture-design.md`](docs/superpowers/specs/2026-09-05-execution-architecture-design.md). AI coding agents must begin with [`AGENTS.md`](AGENTS.md).

## Monetary design

- Native asset: OREG
- Base units per OREG: 100,000,000
- Maximum scheduled supply envelope: 1,000,000 OREG
- Founder allocation: 50,000 OREG, one-time at height 1
- Initial mining subsidy: 2.375 OREG
- Halving interval: 200,000 blocks
- No continuing founder tax, administrative mint, treasury tax, or fee burn

## Current limits

Oregon now contains a tested P2P transport/session/sync foundation, but it is not yet a runnable public cryptocurrency network. The repository does not yet provide:

- production peer discovery/bootstrap policy such as DNS seeding;
- a node RPC surface or mining RPC;
- a wallet or production spend-authorization scheme;
- activated smart-contract execution, EVM/WASM runtimes, or contract account/state commitments;
- activated token/NFT, DeFi, privacy, bridge/interoperability, or AI/oracle/agent protocol domains;
- orphan transaction handling or replace-by-fee;
- a production genesis block, testnet, or mainnet launch configuration; or
- production founder keys, wallet seeds, or deployment secrets.

The architecture contracts reserve Multi-VM, hybrid-state, universal-envelope, dual-ingress, normalized fee/resource, UTXO-backed execution balance, cross-VM, async-message, and future-domain boundaries for versioned milestones. Those capabilities are architecture decisions, not claims of current implementation.

Do not describe the repository as a launched cryptocurrency or production-ready node.

## Development rules

Each consensus, state, persistence, policy, networking, synchronization, and orchestration rule has one authoritative owner. Accepted checkpoints are preserved, milestone work is isolated on development branches, and `main` changes only after a separate integration decision.

Required Rust gates are:

```bash
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

RandomX changes must also pass the architecture-vector and native full/light parity workflows.

## Security

Never commit wallet seeds, private keys, API keys, signing secrets, or other credentials. Unsafe Rust is confined to the RandomX FFI and engine boundary and each unsafe operation is documented with its native ownership, lifetime, pointer, flag, or buffer invariant.
