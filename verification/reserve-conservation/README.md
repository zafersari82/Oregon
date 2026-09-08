# Reserve-conservation verification tooling

The owner approved the design and plan under `docs/superpowers/` on 2026-09-06.
This directory contains a pinned toolchain, live CI bootstrap checks, a standalone
verification-only package and an initial arithmetic/four-slot apply/undo model.
Twelve crate-local production correspondence tests consume this model. RC01–RC10
Kani proofs, the complete production differential matrix, model negative controls
and full proof runner/workflow remain pending. Bootstrap is not reserve proof evidence.

## Toolchain

`toolchain-lock.json` pins the release archive, compiler and verifier binaries,
backend and solver. Its archive SHA-256 was independently computed from the
downloaded bytes and matched the GitHub release asset digest. CaDiCaL 2.0.0 is
embedded in the pinned CBMC binary; it is not a separately downloaded executable.

Install Rust 1.88.0 only for the Kani installer; the production workspace retains
Rust 1.85.0. Install the exact verifier:

```bash
rustup toolchain install 1.88.0 --profile minimal
cargo +1.88.0 install --locked kani-verifier --version 0.67.0
```

Download the asset named in the manifest, verify its size and SHA-256, and then
use `cargo kani setup --use-local-bundle /absolute/path/to/verified-archive.tar.gz`.
The setup installs the pinned nightly named in the manifest. In a root container
that cannot restore archived user ownership, use `TAR_OPTIONS=--no-same-owner`
for that setup command. This affects file ownership metadata, not verifier bytes.
Verify installed binary digests against the manifest before running proofs.

## Bootstrap checks

From the repository root:

```bash
kani verification/reserve-conservation/bootstrap.rs --harness bootstrap_positive
kani verification/reserve-conservation/bootstrap.rs --harness bootstrap_negative -Z concrete-playback --concrete-playback=print
kani verification/reserve-conservation/bootstrap.rs --harness bootstrap_positive
```

The middle command must exit 1 with failure of
`bootstrap_negative.assertion.1`, the named wrapping-increment assertion and
the concrete byte input `255`. Compilation failure or another failed property
does not satisfy this control. The other commands must exit 0 with both assertions
successful and the maximum-input cover satisfied.

Local observed output at source `e816a9c` is retained in `evidence/bootstrap/`.
The summary records the actual commands, source identity and exit codes; absolute
paths describe the observed local execution and can differ on a fresh checkout.
These fixtures describe that historical local run. Later live CI and partial
integration evidence are recorded in
`docs/checkpoints/OREGON_RESERVE_TOOLING_MAIN_INTEGRATION.md`.

## September 7 runner continuation

`python3 -m unittest discover -s scripts -p test_verify_reserve_proofs.py` tests
captured real Kani output, 19 original corrupted-output variants, 14 additional
output-inventory corruptions, and process failure paths (11 test methods).
These Python tests do not execute Kani or establish any reserve property.

The bootstrap-only runner requires a clean checkout and explicit pinned tools:

```bash
python3 scripts/verify_reserve_proofs.py --bootstrap \
  --kani-home /absolute/path/to/kani-0.67.0 \
  --archive /absolute/path/to/verified-archive.tar.gz \
  --rustc /absolute/path/to/pinned-nightly/bin/rustc \
  --output /absolute/path/outside-checkout/new-evidence-directory
```

It checks archive/binary identities, tool versions, exact smoke harness/property
inventory, successful reachability and the named 255 counterexample, then reruns
the positive check. Timeouts kill the process group; partial output is retained.
The evidence directory must be new and outside the checkout. Calling without
`--bootstrap` fails because RC01–RC10 execution is not implemented.

Live pinned bootstrap passed on PR #21 head `51659a39dd0c41bbf607e75f2a4d5b3589e53f27`
in run `34132383672`. Later changes must receive their own exact-head CI. Local
Rust/Kani remain absent in the evidence-gate continuation; no local live verifier
execution is claimed. `evidence/runner/summary.json` is historical runner evidence.
