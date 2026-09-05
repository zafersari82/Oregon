# Oregon M6 Mutation Results — 2026-09-05

## Clean baseline

All five throwaway mutations were created from the exact clean implementation SHA:

`a1c532de5e093c59c6c639a50a39ba9235b7ae8e`

The clean baseline passed the complete Oregon Rust CI gate before mutation work: architecture scan, `cargo test --locked --workspace --all-targets`, chainstate rustdoc with warnings denied, workspace docs, rustfmt, and workspace clippy with warnings denied.

Mutation code was never merged into the M6 implementation branch. Draft mutation PRs #2–#6 were closed after evidence collection, and their branch refs were force-reset to the clean baseline SHA. The mutation commit objects and CI logs remain as audit evidence.

## Mutation 1 — validation-before-relay bypass

- Mutation production SHA: `737112eca8af268f9d576e2f79c904e34c82afdf`
- Change: `validated_relay()` returned a relay authorization for rejected block/transaction results.
- Full mutation CI run: `33960852601`
- Job: `101292363294`
- Killed by:
  - `relay_tests::invalid_block_never_authorizes_inventory_relay`
  - `relay_tests::rejected_transaction_never_authorizes_inventory_relay`
- Result: **KILLED**. Rejected core outcomes could not silently gain relay authority without the relay tests failing.

## Mutation 2 — frame allocation before size validation

- Mutation production SHA: `6f88470529f3f95134cc156b213cb7dba321a56a`
- Change: transport allocated/read the declared payload before decoding and validating the frame header and payload limit.
- Targeted CI run: `33961122813`
- Job: `101293064065`
- Killed by: `tests::oversized_header_is_rejected_before_waiting_for_payload`
- Failure evidence: the test timed out because the mutant waited for the oversized payload instead of rejecting the oversized header before allocation/read.
- Result: **KILLED**.

## Mutation 3 — application traffic before Established

- Mutation production SHA: `cad35876d3aed3ad30a952eabf7df333d642bc1b`
- Change: the handshake state machine accepted/dropped pre-established application traffic instead of returning `HandshakeViolation`.
- Targeted CI run: `33961127675`
- Job: `101293078474`
- Killed by: `tests::pre_established_gossip_is_a_handshake_violation`
- Failure evidence: the test expected an error but received `Ok(None)`.
- Result: **KILLED**.

## Mutation 4 — global sync in-flight cap bypass

- Mutation production SHA: `e86942e966fbb0d3cf08c93bbe5f3f5240fb956f`
- Change: the global scheduler stop condition changed from `>= MAX_IN_FLIGHT_BLOCKS_GLOBAL` to `>`, allowing one request beyond the frozen cap.
- Full mutation CI run: `33960868961`
- Job: `101292406802`
- Killed by: `tests::scheduler_enforces_exact_global_peer_buffer_and_attempt_constants`
- Failure evidence: scheduler produced `33` requests where the exact global cap requires `32`.
- Result: **KILLED**.

A first narrow targeted run (`33961132695`, job `101293093377`) was deliberately discarded as mutation evidence because it failed during compilation from a package-level Tokio feature-unification mismatch before executing the target test. Only the full workspace test failure above counts as the mutation kill.

## Mutation 5 — bypass authoritative chainwork preference

- Mutation production SHA: `861f55b0a365d47112551dd628e3d0144c0d2e49`
- Change: every newly accepted competing header was made preferred, bypassing the authoritative cumulative-chainwork comparison.
- Targeted CI run: `33961138708`
- Job: `101293109564`
- Killed by: `remote_advertised_height_cannot_override_chainstate_preferred_fork_choice`
- Failure evidence: the mutant selected the remote two-block fork as preferred instead of retaining the local three-block authoritative preferred tip.
- Result: **KILLED**.

The full mutation suite (`33960874112`, job `101292420373`) independently failed even earlier at `header_import_tests::lower_work_valid_header_is_stored_without_replacing_preferred_tip`, where the mutant returned `Preferred` instead of `Stored`.

## Result

Mutation score for the five required M6 security experiments: **5 / 5 killed**.

The tests independently guard:

1. validation-before-relay authorization;
2. frame-size validation before attacker-sized allocation/read;
3. handshake gating of application traffic;
4. exact bounded block-sync in-flight ownership;
5. authoritative local chainwork preference over remote height/incoming-fork claims.

No mutation code is part of the clean M6 implementation branch.
