# Reserve-conservation verification tooling

The owner approved the design and plan under `docs/superpowers/` on 2026-09-06.
This directory currently contains a pinned toolchain and bootstrap checks.
The reserve model, RC01–RC10 proofs, production differential matrix, full runner
and CI workflow are still pending. Bootstrap results are not reserve proof evidence.

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
No GitHub proof run, production mutation gate or reserve checkpoint is claimed.

## September 7 runner continuation

`python3 -m unittest discover -s scripts -p test_verify_reserve_proofs.py` tests
captured real Kani output, 19 corrupted-output variants, and process failure paths.
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

The live invocation and tool preflight have not been exercised in this continuation:
Rust/Kani were absent and the Rust download was stopped at network approval.
See `evidence/runner/summary.json` for actual scope and remaining gates.
