# Oregon Mutation Evidence V1

Oregon Mutation Evidence V1 publishes reproducible, machine-readable evidence for the six existing production mutation authorities and their reviewed set of 68 semantic mutations.

The existing authority runners remain authoritative. This publication layer does not replace their mutation logic; it binds the reviewed inventory to exact runner SHA-256 values, requires a clean Git checkout, records exact commit/tree identity, parses one kill record per reviewed mutation, retains raw logs, and writes a canonical `result-v1.json` only after all six authorities succeed.

## Reproduce

Run from the repository root with Rust 1.85.0 available:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 scripts/verify_mutation_evidence_manifest.py
PYTHONDONTWRITEBYTECODE=1 python3 scripts/publish_mutation_evidence.py --output /tmp/oregon-mutation-evidence
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts -p 'test_mutation_evidence*.py' -v
```

The output path must be outside the repository checkout. A successful publication contains:

- `ea.log`
- `ee.log`
- `cs.log`
- `er.log`
- `fs.log`
- `rj.log`
- `result-v1.json`
- `manifest-v1.json` (the exact bytes bound by the result's manifest digest)

A complete passing result reports exactly six authorities and `68/68` killed mutations and binds the evidence to the exact source commit and tree. Failure paths may retain diagnostic/raw logs but must not leave a canonical passing `result-v1.json`.

## Public claim boundary

Mutation testing demonstrates that the selected injected semantic faults are detected by the bound tests under the recorded source and toolchain. For those selected faults, this is stronger evidence than line coverage alone because the tests must detect concrete weakened behaviors.

It is **not** formal verification, exhaustive fault coverage, proof of the absence of bugs, or a claim that every possible implementation defect is detected. The published claim is limited to the reviewed V1 mutation inventory and the exact evidence source recorded in `result-v1.json`.
