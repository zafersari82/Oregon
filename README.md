# Oregon (OREG)

Oregon is an experimental, independent proof-of-work blockchain protocol implemented in Rust. The accepted development baseline currently covers the protocol foundation through the policy-only mempool milestone (M0-M5).

The repository is a clean-room implementation. Bitcoin and earlier Radium materials were used only as historical research inputs; Oregon does not import or patch their implementation code.

## Implemented baseline

The accepted M5 baseline includes:

- canonical transaction and block encoding, identifiers, Merkle commitments, and decode limits;
- fixed monetary representation, founder allocation, mining emission, and halving rules;
- header validation, ASERT difficulty adjustment, targets, and cumulative chainwork;
- Oregon-bound RandomX proof of work with frozen key scheduling and full/light parity vectors;
- UTXO transitions, mandatory spend-verifier boundary, exact coinbase maturity, and undo data;
- RocksDB persistence with atomic batches, WAL, synchronous accepted-state commits, recovery checks, migration policy, and pruning;
- active-chain extension, strictly-heavier-work reorganization, bounded reorg policy, and fail-closed recovery; and
- a deterministic policy mempool with one spender per outpoint, no RBF, no orphan pool, bounded dependency graphs, eviction, and atomic chain reconciliation.

Accepted milestone records are under [`docs/checkpoints`](docs/checkpoints). The current architectural contract is [`docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md`](docs/architecture/OREGON_ENGINEERING_CONSTITUTION.md).

## Monetary design

- Native asset: OREG
- Base units per OREG: 100,000,000
- Maximum scheduled supply envelope: 1,000,000 OREG
- Founder allocation: 50,000 OREG, one-time at height 1
- Initial mining subsidy: 2.375 OREG
- Halving interval: 200,000 blocks
- No continuing founder tax, administrative mint, treasury tax, or fee burn

## Current limits

Oregon is not a runnable public network. The repository does not yet provide:

- peer-to-peer networking or peer discovery;
- a node RPC surface or mining RPC;
- a wallet or production spend-authorization scheme;
- orphan transaction handling or replace-by-fee;
- a production genesis block, testnet, or mainnet launch configuration; or
- production founder keys, wallet seeds, or deployment secrets.

Do not describe the repository as a launched cryptocurrency or production-ready node.

## Development rules

Each consensus, state, persistence, and policy rule has one authoritative owner. Accepted checkpoints are preserved, milestone work is isolated on development branches, and `main` changes only after a separate integration decision.

Required Rust gates are:

```bash
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

RandomX changes must also pass the architecture-vector and native full/light parity workflows.

## Security

Never commit wallet seeds, private keys, API keys, signing secrets, or other credentials. Unsafe Rust is confined to the RandomX FFI and engine boundary.
