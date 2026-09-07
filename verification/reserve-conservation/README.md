# Reserve-conservation verification tooling

The owner approved the design and plan under `docs/superpowers/` on 2026-09-06.
This directory currently contains a pinned toolchain and bootstrap checks.
The reserve model, RC01–RC10 proofs, production differential matrix and CI workflow
are still pending. The fail-closed repository runner is present, but refuses the
incomplete RC suite. Bootstrap results are not reserve proof evidence.

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
python3 scripts/verify_reserve_proofs.py --bootstrap
```

The runner verifies the pinned Kani, CBMC, compiler and toolchain identities before
executing the exact manifest-listed sequence. The middle Kani invocation must exit 1
with failure of
`bootstrap_negative.assertion.1`, the named wrapping-increment assertion and
the concrete byte input `255`. Compilation failure or another failed property
does not satisfy this control. The other commands must exit 0 with both assertions
successful and the maximum-input cover satisfied.

Local observed output at source `e816a9c` is retained in `evidence/bootstrap/`.
The summary records the actual commands, source identity and exit codes; absolute
paths describe the observed local execution and can differ on a fresh checkout.
No GitHub proof run, production mutation gate or reserve checkpoint is claimed.
