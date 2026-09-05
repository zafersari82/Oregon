# Oregon v1 M6 Network and Node Checkpoint

Date: 2026-09-05
Implementation branch: `design/m6-network-architecture-2026-09-04`
Accepted M5/main base before M6 integration: `4e2d249a3e839e3cea3e7b1b7b0e87d327189947`
Final verified M6 head: `1b3040f8346036a85d33d4fb291ef68c83945155`
Accepted M6 `main` integration commit: `748989da8009cc36fb07f82465a03dcb92006c58`
Accepted tree: `85939c202ed3ee7bc9fc0cc5dde5f44fd538e81e`

## Accepted scope

M6 adds Oregon's bounded wire/network/peer/synchronization/node-orchestration foundation while preserving the accepted M0-M5 consensus, RandomX, UTXO, storage, chainstate, and mempool rule ownership.

Accepted M6 behavior includes:

- `oregon-protocol` owns protocol-version and feature negotiation, canonical wire messages, inventory representation, frame headers/checksums, and remote message/resource limits.
- `oregon-network` owns the transport abstraction and production TCP transport, framed reads/writes, payload-size rejection before attacker-sized reads, and bounded I/O deadlines.
- `oregon-peer` owns handshake lifecycle, self-peer detection, deterministic simultaneous-duplicate arbitration, bounded queues/byte reservations, request matching, liveness, peer scoring, and cooldown policy.
- `oregon-sync` owns headers-first locator/synchronization mechanics and bounded block scheduling while consuming only authoritative local chain/preferred-header views through `ChainSyncView`.
- Remote advertised height is metadata only and cannot override authoritative local cumulative-chainwork preference.
- The block scheduler enforces the frozen global/per-peer ownership bounds, timeout reassignment, deterministic peer preference, buffering, and the third-failed-attempt `Stalled` state.
- `oregon-node` is orchestration only. A single blocking core owner contains mutable `ChainState` and `Mempool` state; network/sync code does not duplicate consensus or mempool policy.
- Node core commands are bounded by an exact command-count queue and byte semaphore budget. Remote header batches are sliced into the exact core validation bound before submission.
- `NodeSyncView` exposes coarse synchronization reads through bounded core commands and does not leak storage/chainstate error types below the node boundary.
- Blocks and transactions are relay-eligible only after authoritative ChainState/Mempool acceptance produces an opaque `ValidatedRelay` authorization.
- Per-peer known-inventory and recent-relay caches are bounded and deterministic; source and already-known peers are excluded from relay.
- Object requests register `Expect` before sending `GetData`, preserving response matching under fast peers.
- Storage-faulted/reindex-required states stop new mutation/synchronization work without converting local state failure into peer blame.
- Core crates remain independent of M6 networking crates; CI enforces the upward-dependency prohibition.
- Unsafe Rust remains confined to the RandomX boundary.

M6 intentionally does not add node RPC/mining RPC, a wallet, production spend authorization, orphan/RBF/package policy, production peer discovery/DNS seeding, a production genesis block, public testnet, mainnet configuration, production keys, or deployment secrets.

## Real integration and resilience evidence

The accepted test suite includes real loopback TCP integration coverage for:

- three-node session establishment with negotiated features;
- self-connection rejection by process nonce;
- simultaneous duplicate TCP dials selecting the same physical direction;
- accepted block relay only after ChainState acceptance;
- accepted transaction relay only after Mempool admission, with conflicting-spend non-relay;
- a behind node catching up headers-first through a validating middle peer;
- fork/lying-height resistance where remote advertised height cannot replace local authoritative chainwork choice;
- exact 20-second nonresponse reassignment and third-timeout `Stalled` behavior; and
- remote header batches larger than the core slice being split before authoritative submission.

The workspace also retains exact unit/contract coverage for protocol decode limits, network deadlines, peer queue bounds, handshake gates, request/grace behavior, scoring/cooldown, synchronization locator/topology, scheduler ownership limits, node core byte/command bounds, sync adapter isolation, inventory bounds, and validation-before-relay authorization.

## CI architecture boundaries

The accepted M6 CI architecture scan rejects:

- networking dependencies from `oregon-consensus`, `oregon-utxo`, `oregon-storage`, `oregon-chainstate`, or `oregon-mempool`;
- attacker-declared transaction/input/output/witness counts driving direct vector preallocation;
- founder allocation, subsidy, RandomX key derivation, or storage column-family rule leakage into `oregon-protocol`, `oregon-network`, `oregon-peer`, or `oregon-sync`;
- storage representation outside `oregon-storage`;
- unsafe Rust outside `oregon-pow`; and
- superseded task-numbered/legacy architecture symbols.

## Clean verification evidence

Final verified implementation head:

`1b3040f8346036a85d33d4fb291ef68c83945155`

Oregon Rust CI run `33962331751`, job `101296256903`:

- architecture scan: SUCCESS
- `cargo +1.85.0 test --locked --workspace --all-targets`: SUCCESS
- chainstate rustdoc with warnings denied: SUCCESS
- workspace docs: SUCCESS
- `cargo +1.85.0 fmt --all -- --check`: SUCCESS
- `cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings`: SUCCESS

The accepted squash integration commit `748989da8009cc36fb07f82465a03dcb92006c58` has the exact same tree SHA (`85939c202ed3ee7bc9fc0cc5dde5f44fd538e81e`) as the verified implementation head, so the merge did not alter source content.

## Required M6 mutation evidence

The canonical mutation report is:

`docs/superpowers/mutations/2026-09-05-oregon-m6-mutation-results.md`

All five required throwaway security mutations were killed:

1. validation-before-relay bypass — KILLED;
2. frame allocation/read before size validation — KILLED;
3. application traffic before established handshake — KILLED;
4. global sync in-flight cap bypass — KILLED;
5. authoritative chainwork-preference bypass — KILLED.

Mutation score: **5 / 5 killed**.

Mutation source was never merged into the clean M6 implementation. The report preserves the exact mutation SHAs, GitHub Actions run/job IDs, killed tests, and observed failure evidence.

## Acceptance

M6 is accepted as Oregon's bounded protocol/network/peer/sync/node-orchestration foundation at verified source tree `85939c202ed3ee7bc9fc0cc5dde5f44fd538e81e`, integrated to `main` by commit `748989da8009cc36fb07f82465a03dcb92006c58`.

Acceptance means the networking and synchronization foundation is part of Oregon's official baseline and is subject to the same one-owner, fail-closed, bounded-resource, validation-before-publication/relay, test-first, and no-shim architecture rules as M0-M5.

This checkpoint does **not** claim that Oregon is a launched cryptocurrency, a production-ready public node, or mainnet-ready. RPC/mining, wallet/production spend authorization, production peer discovery/bootstrap, genesis/testnet/mainnet configuration, deployment security, and later launch-readiness work remain outside M6.
