# Oregon v1 M5 Mempool Design

Date: 2026-09-04
Status: in-chat design approved; written spec awaiting user review
Development branch: `oregon-v1-m5-mempool`
Accepted base: `6ff8168bb79b0f7e1aa015ce910cedaf108614ae` (accepted M4 persistent-chainstate checkpoint)

## 1. Goal

M5 adds an in-memory transaction mempool and admission/policy layer to Oregon on top of the accepted M3 UTXO transition engine and M4 persistent chainstate.

The principal rule is:

> M5 must not create a second consensus engine. A transaction may enter the mempool only after the existing Oregon transaction/UTXO validation boundaries accept it against the current active-chain state plus its already-accepted unconfirmed ancestors.

M5 provides the transaction pool needed before P2P relay, miner/block-template construction, and RPC integration can be designed cleanly.

M5 remains a node-policy milestone, not a consensus-rule milestone. Mempool contents are not consensus state and are not persisted to RocksDB in this milestone.

## 2. Explicit non-goals

M5 does not implement:

- P2P networking or transaction relay
- orphan transaction storage
- Replace-By-Fee (RBF)
- package relay or CPFP package-feerate policy
- mempool persistence across process restart
- wallet/address support
- mining RPC or block-template RPC
- automatic resurrection of transactions disconnected by a reorg
- production Schnorr/KeyCommitV1 cryptography beyond the existing mandatory `SpendVerifier` boundary
- new lock-time or sequence consensus semantics
- mainnet/testnet launch behavior

A child submitted before its unconfirmed parent is rejected for now and may be retried after the parent is accepted. There is no orphan queue in M5.

## 3. Architecture

M5 introduces one focused crate:

- `oregon-mempool`: transaction admission, dependency tracking, conflict tracking, memory limits, deterministic ordering/eviction, active-chain reconciliation, and mempool-specific typed errors.

The crate depends on:

- `oregon-primitives` for canonical `Transaction`, `OutPoint`, `Hash256`, and transaction encoding/txid
- `oregon-consensus` for shared transaction structural consensus checks and the authoritative maximum transaction size
- `oregon-utxo` for `UtxoState`, `UtxoEntry`, `SpendVerifier`, and the existing atomic normal-transaction transition logic

`oregon-mempool` does not own RocksDB and does not depend on `oregon-storage`.

`oregon-mempool` also does not need to depend on `oregon-chainstate`. A future node orchestration layer will pass the active tip identity/height and an immutable reference to the current `UtxoState` using the already-public `ChainState::tip()` and `ChainState::utxos()` boundaries.

This keeps dependency direction one-way and prevents chainstate consensus code from importing mempool policy.

## 4. Shared normal-transaction structural validation

The accepted block validator already enforces structural rules for every non-coinbase transaction:

- encoded transaction size must not exceed the consensus maximum
- normal transactions must have at least one input
- normal transactions must have at least one output
- coinbase-form transactions are not valid normal transactions
- a normal transaction may not contain the null coinbase outpoint

M5 must not copy these consensus-shape rules into a second independent implementation.

Implementation may refactor `oregon-consensus` to expose a small shared normal-transaction skeleton validator. The block skeleton validator must call the same authoritative helper so block validation and mempool admission cannot drift.

The refactor must preserve the exact existing block-level behavior and error meaning. It is not permission to weaken or reinterpret consensus rules.

The standalone helper may use a dedicated small structural error type if necessary so block validation can attach transaction indexes without forcing mempool policy to invent a fake block index.

## 5. Mempool entry model

Each accepted transaction is represented by a `MempoolEntry` containing at least:

- canonical transaction object
- `txid`
- validated transaction fee in base units
- canonical encoded byte length
- direct unconfirmed parent txids
- direct unconfirmed child txids

The pool maintains:

- entries keyed by txid
- an outpoint-spend index mapping each currently claimed input outpoint to exactly one mempool txid
- total canonical encoded bytes
- direct parent/child dependency edges
- the active-chain base identity the pool has most recently reconciled against: block id and height

Consensus-visible semantics must never depend on a Rust `HashMap` iteration order. Where ordering is externally observed, tested, used for eviction, or used for replay, the implementation uses explicit ordering such as `BTreeMap`, `BTreeSet`, or sorted vectors.

`HashMap`/`HashSet` may still be used for lookup-only internal indexes when iteration order has no semantic effect.

## 6. Chain base and stale-context protection

A mempool is always associated with one active-chain base:

- active tip block id
- active tip height

Admission requires the caller to provide the current chain tip identity/height and immutable UTXO view.

If the provided tip identity/height does not match the mempool's last reconciled base, admission fails with an explicit stale-context error. The caller must first reconcile/revalidate the mempool against the new chain state.

This prevents a transaction from being accepted using a mempool overlay derived from an obsolete active tip.

The next possible spend height used for mempool validation is `tip_height + 1`, using checked arithmetic. This makes coinbase maturity policy match the earliest block in which the mempool transaction could be mined.

## 7. Admission pipeline

Admission is all-or-nothing. No txid entry, spend-index claim, dependency edge, byte counter, or eviction result becomes visible until every required validation step succeeds.

For a candidate transaction:

1. Compute canonical encoded bytes and txid using the existing primitive implementation.
2. Apply shared normal-transaction structural validation.
3. Reject an already-known txid.
4. Verify chain base identity/height matches the mempool's current base.
5. Identify direct unconfirmed parents from candidate inputs whose `previous_txid` exists in the current mempool.
6. For each input that refers to an existing mempool parent, verify the referenced output index actually exists in that parent transaction.
7. Reject any input whose outpoint is already claimed by another mempool transaction.
8. If an input is neither available in active-chain UTXO state nor produced by an existing mempool parent, reject it as a missing dependency/orphan. M5 does not retain the candidate.
9. Compute the candidate's complete unconfirmed ancestor closure.
10. Enforce ancestor/descendant policy limits before publication.
11. Build a temporary validation UTXO state containing only the chain UTXOs needed by the ancestor closure and candidate.
12. Replay ancestors in deterministic topological order through the existing `UtxoState::apply_normal_transaction()` path with the required caller-supplied `SpendVerifier`.
13. Apply the candidate through the same existing UTXO transition path. The returned fee is the authoritative candidate fee.
14. Stage insertion of the candidate entry, spend claims, dependency edges, byte totals, and any capacity eviction required by policy.
15. If capacity handling would evict the new candidate itself, reject admission and publish none of the staged pool changes.
16. Otherwise commit the complete staged mempool state atomically.

The admission path never uses a production `AcceptAll` verifier.

## 8. Narrow validation overlay

M5 must avoid cloning the complete chain UTXO set for every transaction and must also avoid writing a second UTXO transition algorithm.

For candidate validation, M5 constructs a narrow temporary `UtxoState` using the existing checked `UtxoState::from_persisted_entries(...)` restoration boundary:

- collect the candidate plus all already-accepted unconfirmed ancestors
- inspect their inputs
- seed the temporary state with each referenced outpoint that currently exists in the active-chain `UtxoState`
- do not seed outputs that are created by unconfirmed ancestors; those outputs are created naturally when ancestors are replayed
- replay ancestors parent-before-child
- finally apply the candidate

This preserves the exact M3 duplicate-input, coinbase-maturity, amount arithmetic, fee, output-collision, and mandatory `SpendVerifier` semantics without cloning the entire active UTXO set.

If an ancestor replay unexpectedly fails, admission fails closed. The existing pool is not mutated.

## 9. Dependency graph rules

The M5 mempool is a directed acyclic dependency graph where an edge `parent -> child` exists when a child input spends an output produced by the parent transaction.

Because child-before-parent submission is rejected, normal admission cannot create a dependency on a transaction that is not already present.

The implementation still treats an observed dependency cycle during internal traversal/revalidation as corruption/invariant failure rather than attempting to guess a repair.

Direct parent and child sets are stored explicitly and updated atomically with the entry.

Removing a transaction for invalidity/conflict/eviction normally removes all of its current descendants unless a special chain-confirmation rule explicitly promotes those descendants to chain-backed inputs.

## 10. Conflict policy

M5 supports exactly one mempool spender per outpoint.

If candidate input outpoint `X` is already claimed by transaction `A`, a different transaction `B` spending `X` is rejected with a typed conflict error.

There is no RBF in M5. Fee amount, fee rate, age, or txid cannot replace an existing conflicting transaction through normal admission.

A block that becomes active may legitimately spend an outpoint claimed by a mempool transaction. Active-chain state wins. Reconciliation removes the conflicting mempool transaction and its descendants.

## 11. Fees and canonical size

A mempool entry fee is the fee returned by the accepted `UtxoState::apply_normal_transaction()` validation path.

Transaction byte size is `transaction.encode().len()` using canonical protocol bytes, including witness because witness already commits to the Oregon txid in v0.

No floating-point fee-rate arithmetic is used.

When a deterministic fee-rate comparison is needed, compare the rational values using integer cross multiplication in a sufficiently wide checked domain, e.g. `u128`:

`fee_a / bytes_a` vs `fee_b / bytes_b`

by comparing:

`fee_a * bytes_b` vs `fee_b * bytes_a`.

The implementation must handle arithmetic conversion/checking explicitly and must not silently wrap.

M5 does not introduce a mandatory non-zero relay fee because there is not yet a P2P relay layer. Zero-fee transactions may be admitted if otherwise valid and capacity allows.

## 12. Memory limits

M5 is strictly bounded. The default policy is configurable and non-consensus.

Initial default limits for the implementation plan:

- maximum mempool entries: `50_000`
- maximum sum of canonical encoded transaction bytes: `64 MiB`
- maximum unconfirmed ancestors per transaction: `25`
- maximum unconfirmed descendants per transaction: `25`
- per-transaction byte ceiling: the existing consensus `MAX_TRANSACTION_BYTES` (`102_400` bytes)

The entry-count ceiling is required in addition to encoded-byte accounting because Rust object/index/dependency overhead is larger than raw transaction bytes and tiny transactions must not create unbounded metadata growth.

These are node-policy defaults, not consensus constants, and may later become runtime configuration.

## 13. Deterministic capacity eviction

When a valid candidate would exceed a hard pool bound, admission evaluates capacity on staged state.

Eviction is deterministic and dependency-safe:

1. Select the lowest individual fee-rate transaction using exact integer comparison.
2. Break equal fee-rate ties by lower absolute fee.
3. Break remaining ties lexicographically by txid bytes.
4. Evict the selected transaction together with all of its descendants so no retained transaction depends on a missing mempool parent.
5. Repeat until all hard limits are satisfied.

M5 intentionally does not implement CPFP/package-feerate scoring. Therefore a low-fee parent may cause higher-fee descendants to be removed as part of its dependency subtree. Package economics are deferred to a later policy milestone.

If deterministic capacity eviction selects the newly submitted candidate (directly or as a descendant of another selected transaction), admission returns a capacity-rejected result and commits none of the staged state changes. The pre-admission pool remains byte-for-byte/logically unchanged.

## 14. Deterministic topological order

M5 exposes a read-only deterministic parent-before-child transaction order for testing and future miner/node integration.

Topological traversal uses explicit sorted-ready ordering. When several transactions are simultaneously dependency-free, txid byte order is the deterministic tie-breaker.

This order is not a consensus block-ordering requirement and is not yet a miner fee-selection algorithm. Its purpose is to provide a stable, dependency-valid traversal and replay order.

Future mining policy may add fee-aware block-template selection without changing M5 admission validity.

## 15. Active-block reconciliation

After a block becomes part of the active chain, the caller supplies the accepted block and the new chain tip/UTXO state to M5.

Reconciliation distinguishes confirmation from conflict.

### Confirmed mempool transactions

If a non-coinbase transaction from the active block already exists in the mempool:

- remove that mempool entry
- remove its spend-index claims
- do **not** automatically remove its descendants merely because the parent entry disappeared
- remove the parent dependency edge from those children

The parent's outputs now exist in active-chain UTXO state, so previously unconfirmed children may remain valid as chain-backed transactions.

### Active-block conflicts

For each active-block normal input, if the same outpoint is claimed by a different remaining mempool transaction:

- remove the conflicting mempool transaction
- remove all of its descendants

The active chain is authoritative.

### Full revalidation

After confirmation/conflict processing, rebuild/revalidate the remaining mempool deterministically against the new chain UTXO snapshot and next spend height.

Any transaction that is no longer valid is removed together with descendants that cannot independently remain valid.

Only after successful reconciliation is the mempool base tip identity/height advanced to the new chain tip.

## 16. Reorg reconciliation

For any active-chain change where a simple connected-block delta is not sufficient, including a reorg, M5 performs full deterministic revalidation of the currently retained mempool against the new active-chain UTXO snapshot and next spend height.

Transactions that remain valid are retained. Transactions invalidated by the new active chain are removed, with dependent descendants removed or re-evaluated according to actual chain-backed availability.

M5 does **not** automatically resurrect normal transactions from disconnected old-chain blocks. Reorg resurrection requires the node/orchestration layer to supply disconnected transaction bodies and define replay/relay policy; that is explicitly deferred.

If a previously confirmed parent disappears during a reorg and is not currently in the mempool or active UTXO set, a retained child depending on that parent becomes missing-input/orphan and is removed during revalidation.

## 17. Full-pool revalidation algorithm

Full revalidation must be deterministic and fail closed.

A clean implementation strategy is:

1. Snapshot current entries by txid and dependency metadata.
2. Determine deterministic parent-before-child order.
3. Seed one temporary `UtxoState` with every chain outpoint referenced by retained transactions that exists in the new active-chain UTXO set.
4. Replay transactions in topological order through `apply_normal_transaction()` at `new_tip_height + 1`.
5. If a transaction fails, mark it invalid and do not apply it to the overlay.
6. Transactions depending on a failed/missing parent will naturally fail or be removed as descendants.
7. Rebuild entries, parent/child edges, spend index, total bytes, and capacity state from only successfully retained transactions.
8. Publish the rebuilt mempool and new base identity atomically.

An unexpected internal graph cycle or impossible bookkeeping inconsistency returns an invariant error and leaves the old mempool/base unchanged rather than publishing a partially rebuilt state.

## 18. Error model

`oregon-mempool` exposes typed policy/state errors and preserves underlying consensus/UTXO causes where useful.

Required distinctions include equivalents of:

- `AlreadyKnown(txid)`
- `StaleChainContext`
- `Conflict { outpoint, existing_txid }`
- `MissingDependency(outpoint)`
- `InvalidParentOutput(outpoint)`
- `TooManyAncestors`
- `TooManyDescendants`
- `CapacityRejected`
- `DependencyCycle`
- `InvariantViolation`
- structural consensus rejection
- `Utxo(UtxoError)` / spend authorization failure

Consensus-invalid transaction failure remains distinct from local mempool capacity/policy rejection.

## 19. Atomicity and failure behavior

Every externally visible M5 mutation is staged before publication.

Admission failure must leave unchanged:

- entries
- spend index
- parent/child graph
- total bytes
- base tip identity

Reconciliation failure caused by an internal invariant problem must also leave the previous mempool/base unchanged.

Expected transaction invalidity during full revalidation is not an internal reconciliation failure: invalid entries are intentionally filtered from a staged rebuilt pool and the final valid subset may then be published atomically.

No partial index updates are allowed.

## 20. Interaction with `SpendVerifier`

M5 inherits the M3 rule that every normal input must pass a caller-supplied `SpendVerifier`.

The same verifier boundary is used when:

- validating a new candidate
- replaying unconfirmed ancestors for candidate validation
- revalidating the full mempool after active-chain changes

M5 does not provide an exported permissive production verifier.

Test-only verifiers may accept/reject deterministically under `#[cfg(test)]` or test modules.

When production Schnorr/KeyCommitV1 authorization is implemented later, it can satisfy the same verifier contract without redesigning mempool admission.

## 21. Lock-time and sequence treatment

The current accepted Oregon consensus model carries `lock_time` and input `sequence` fields but M3/M4 do not define additional lock-time/sequence spend-validity semantics.

M5 therefore does not invent policy that would accidentally become a shadow consensus rule.

Those fields remain committed in canonical transaction bytes/txid and pass through existing validation. If a future consensus milestone defines finality/relative-lock semantics, M5 must then call the authoritative consensus helper and revalidation must incorporate those rules.

## 22. TDD acceptance matrix

Implementation follows strict RED -> observed failure -> minimal GREEN -> fresh full verification.

M5 acceptance tests include at least:

1. A valid chain-backed transaction is admitted with exact fee and canonical byte length.
2. Exact txid duplicate is rejected without pool mutation.
3. Two different transactions spending the same chain outpoint conflict; the second is rejected.
4. Parent then child admission succeeds and child validation sees the unconfirmed parent output.
5. Child-before-parent is rejected and is not retained as an orphan.
6. A child referencing an out-of-range parent output index is rejected.
7. Missing chain/mempool input is rejected without mutation.
8. Existing M3 `SpendVerifier` rejection prevents admission and leaves all pool indexes unchanged.
9. Coinbase maturity uses `tip_height + 1` and preserves the exact 120-block boundary.
10. Normal-transaction structural rules used by block validation and mempool admission remain in parity.
11. Consensus maximum transaction bytes are enforced.
12. Ancestor limit is enforced exactly at the configured boundary.
13. Descendant limit is enforced exactly at the configured boundary.
14. Deterministic topological traversal always emits parents before children and uses txid tie-breaks.
15. Different insertion orders that produce the same independent transaction set yield the same deterministic topological order and eviction choice.
16. Capacity pressure evicts the lowest fee-rate candidate/subtree using exact integer fee-rate comparison.
17. Equal fee-rate eviction uses absolute fee then txid tie-breaks.
18. If staged capacity handling would evict the new candidate, the original pool remains unchanged.
19. Confirmed mempool parent removal preserves a valid child whose input is now chain-backed.
20. Active block conflict removes the conflicting mempool transaction and descendants.
21. Full revalidation after a normal active-tip change preserves valid entries and removes newly invalid ones.
22. Full revalidation after a reorg removes children whose formerly confirmed parent output disappeared.
23. Reorg reconciliation does not invent/resurrect disconnected transactions that were not already in the pool.
24. Stale chain tip context rejects new admission until reconciliation occurs.
25. Internal graph-cycle/invariant failure does not publish partial rebuilt state.
26. No externally visible ordering depends on `HashMap` iteration.
27. Workspace tests, rustfmt, and clippy `-D warnings` remain green with M1-M4 regression suites unchanged.

## 23. Required security mutations

M5 is not accepted until targeted throwaway mutations are killed by intended tests.

### Mutation A — double-spend conflict bypass

Disable or bypass the outpoint-spend conflict check so two different mempool transactions can claim the same outpoint.

Required result: conflict/double-spend admission tests fail.

### Mutation B — missing-parent/orphan bypass

Change admission so a child with an input unavailable in both active UTXO state and accepted mempool parents is retained/accepted.

Required result: child-before-parent and missing-dependency tests fail.

### Mutation C — early/partial mempool publication

Publish the candidate entry, spend claim, dependency edge, or byte accounting before final UTXO/SpendVerifier/capacity validation succeeds.

Required result: failure-atomicity/state-equality tests fail.

All mutation code lives on throwaway branches and must never enter the accepted M5 branch.

## 24. Manual security review scope

Before M5 acceptance, manual review covers at least:

- no duplicate consensus transaction-shape implementation drift
- mandatory `SpendVerifier` use on candidate, ancestor replay, and revalidation paths
- no exported production permissive verifier
- no unchecked amount/fee arithmetic
- exact canonical encoded-byte accounting
- no floating-point fee-rate behavior
- one-spender-per-outpoint invariant
- dependency graph parent/child bookkeeping atomicity
- child-before-parent/orphan policy
- ancestor/descendant limit off-by-one boundaries
- capacity bound enforcement and candidate-self-eviction rollback
- deterministic ordering and eviction tie-breaks
- confirmed-parent child promotion correctness
- active-block conflict descendant removal
- stale-chain-context rejection
- full revalidation publication atomicity
- reorg behavior and explicit non-resurrection scope
- no `HashMap` iteration ordering leaking into externally observed policy
- M3 coinbase maturity and UTXO semantics unchanged
- M4 chainstate/storage durability semantics unchanged

No Critical or Important finding may remain open at acceptance.

## 25. CI and branch discipline

M5 implementation starts from accepted M4 checkpoint:

`6ff8168bb79b0f7e1aa015ce910cedaf108614ae`

Development branch:

`oregon-v1-m5-mempool`

During implementation the Oregon Rust CI workflow may add this milestone branch to push triggers. Temporary mutation-branch triggers must be removed from the clean accepted tree.

Every implementation task ends with fresh:

- `cargo +1.85.0 test --locked --workspace --all-targets`
- `cargo +1.85.0 fmt --all -- --check`
- `cargo +1.85.0 clippy --locked --workspace --all-targets -- -D warnings`

M5 acceptance requires a fresh pre-mutation clean gate, mutation evidence, and a fresh post-mutation clean gate on the reviewed source tree.

## 26. Definition of M5 accepted

M5 is accepted only when all of the following hold at a reviewed checkpoint:

- `oregon-mempool` implements the approved admission, graph, conflict, limits, deterministic ordering/eviction, and chain reconciliation scope
- shared normal-transaction structural validation cannot drift between block and mempool paths
- M1-M4 behavior remains green
- workspace tests pass
- rustfmt passes
- clippy passes with warnings denied
- required M5 security mutations are each killed by intended tests
- fresh post-mutation clean CI passes
- manual M4 -> M5 security review has no known Critical or Important finding left open
- a checkpoint document records exact commit SHAs, CI runs, and mutation evidence
- an accepted recovery branch is created, planned name:
  `oregon-v1-checkpoint-m5-mempool-accepted-2026-09-04`

`main` is not merged or modified as part of M5 implementation or acceptance unless the user later explicitly requests that integration.

## 27. Resulting milestone order

With this M5 accepted, Oregon's architecture becomes:

1. protocol primitives/canonical encoding
2. monetary and consensus rules
3. RandomX PoW
4. UTXO state transition
5. persistent active chainstate/reorg/pruning
6. **mempool transaction admission/policy (M5)**

The next major milestone can then build P2P/network synchronization around a node that already knows how to validate, retain, reconcile, and bound unconfirmed transactions rather than inventing those semantics inside the networking layer.
