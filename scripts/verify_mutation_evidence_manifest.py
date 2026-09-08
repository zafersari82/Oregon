#!/usr/bin/env python3
"""Validate the committed Mutation Evidence Manifest V1."""
import argparse
from pathlib import Path
import sys

from mutation_evidence import EvidenceError, load_manifest


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST = ROOT / "verification/mutation-evidence/manifest-v1.json"


def main(argv=None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    args = parser.parse_args(argv)
    try:
        _, authorities = load_manifest(args.manifest, ROOT)
    except EvidenceError as error:
        print(f"Mutation evidence manifest rejected: {error}", file=sys.stderr)
        return 2
    count = sum(authority.expected_count for authority in authorities)
    print(f"Mutation evidence manifest valid: {len(authorities)} authorities, {count} mutations")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
