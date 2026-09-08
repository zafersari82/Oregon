#!/usr/bin/env python3
"""Temporary read-only inventory helper for populating Mutation Evidence V1."""
import hashlib
import json
from pathlib import Path
import runpy

ROOT = Path(__file__).resolve().parents[1]
RUNNERS = [
    ("EA", "scripts/verify_execution_address_mutations.py"),
    ("EE", "scripts/verify_execution_envelope_mutations.py"),
    ("CS", "scripts/verify_contract_state_mutations.py"),
    ("ER", "scripts/verify_execution_resource_mutations.py"),
    ("FS", "scripts/verify_fee_settlement_mutations.py"),
    ("RJ", "scripts/verify_journal_mutations.py"),
]


def relative(path):
    path = Path(path)
    if not path.is_absolute():
        return path.as_posix()
    return path.resolve().relative_to(ROOT.resolve()).as_posix()


def metadata(namespace, item):
    if hasattr(item, "name"):
        return item.name, relative(item.path), item.expected_test
    size = len(item)
    if size == 3:
        return item[0], relative(namespace["SOURCE"]), item[2]
    if size == 5:
        return item[0], relative(item[1]), item[4]
    if size == 6:
        return item[0], relative(item[1]), item[5]
    if size == 7:
        return item[0], relative(item[1]), item[6]
    raise RuntimeError(f"unsupported mutation tuple shape: {size}")


def main():
    document = []
    for prefix, runner in RUNNERS:
        path = ROOT / runner
        namespace = runpy.run_path(str(path))
        mutations = []
        for index, item in enumerate(namespace["MUTATIONS"], start=1):
            name, target, test = metadata(namespace, item)
            mutations.append({
                "id": f"{prefix}-{index:03d}",
                "name": name,
                "target": target,
                "killing_test": test,
            })
        document.append({
            "id": prefix,
            "runner": runner,
            "runner_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "count": len(mutations),
            "mutations": mutations,
        })
    print(json.dumps(document, sort_keys=True, separators=(",", ":")))


if __name__ == "__main__":
    main()
