"""Prove Stage-2 contract-state tests kill critical consensus mutations.

Run only on a disposable CI checkout. Every production file is restored after
its mutant; compilation failure is never accepted as mutation-kill evidence.
"""

from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
HASH = ROOT / "crates/oregon-contract-state/src/hash.rs"
TRANSITION = ROOT / "crates/oregon-contract-state/src/transition.rs"
SOURCE = ROOT / "crates/oregon-contract-state/src/source.rs"
PROOF = ROOT / "crates/oregon-contract-state/src/proof.rs"
AGGREGATE = ROOT / "crates/oregon-primitives/src/state_commitment.rs"

STATE_COMMAND = [
    "cargo", "+1.85.0", "test", "--locked", "-p", "oregon-contract-state",
    "--test", "smt_vectors", "--test", "security", "--test", "proofs",
]
AGGREGATE_COMMAND = [
    "cargo", "+1.85.0", "test", "--locked", "-p", "oregon-primitives",
    "--test", "state_commitments",
]

MUTATIONS = [
    (
        "path-key hash omits commitment domain",
        HASH,
        [(
            "    let mut payload = Vec::with_capacity(2 + key.len());\n"
            "    payload.extend_from_slice(&domain_prefix(domain));\n"
            "    payload.extend_from_slice(key);\n",
            "    let mut payload = Vec::with_capacity(key.len());\n"
            "    payload.extend_from_slice(key);\n",
        )],
        STATE_COMMAND,
        "identical_raw_key_and_value_are_domain_separated",
    ),
    (
        "path traversal becomes LSB-first",
        HASH,
        [(
            "    Ok((byte & (0x80 >> (depth % 8))) != 0)\n",
            "    Ok((byte & (0x01 << (depth % 8))) != 0)\n",
        )],
        STATE_COMMAND,
        "path_bits_are_msb_first_at_frozen_boundaries",
    ),
    (
        "empty leaf uses zero instead of domain-separated hash",
        HASH,
        [(
            "    hashes[SMT_DEPTH] = domain_hash(EMPTY_DOMAIN, &prefix);\n",
            "    hashes[SMT_DEPTH] = Hash256::from_bytes([0u8; 32]);\n",
        )],
        STATE_COMMAND,
        "literal_vectors_pin_path_value_empty_and_one_leaf_roots",
    ),
    (
        "two-default-child subtree is not collapsed to canonical empty",
        TRANSITION,
        [(
            "        if new_left == self.empty[depth + 1] && new_right == self.empty[depth + 1] {\n",
            "        if false && new_left == self.empty[depth + 1] && new_right == self.empty[depth + 1] {\n",
        )],
        STATE_COMMAND,
        "update_delete_and_read_preserve_immutable_snapshot_semantics",
    ),
    (
        "duplicate state paths are accepted",
        TRANSITION,
        [(
            "            if pair[0].path == pair[1].path {\n",
            "            if false && pair[0].path == pair[1].path {\n",
        )],
        STATE_COMMAND,
        "duplicate_path_and_domain_mismatch_fail_closed",
    ),
    (
        "aggregate descriptor omits domain id",
        AGGREGATE,
        [(
            "        bytes[0..2].copy_from_slice(&u16::from(self.domain_id).to_le_bytes());\n",
            "        // MUTANT: domain id intentionally omitted.\n",
        )],
        AGGREGATE_COMMAND,
        "aggregate_root_binds_domain_scheme_and_child_root",
    ),
    (
        "aggregate descriptor omits commitment scheme id",
        AGGREGATE,
        [(
            "        bytes[2..4].copy_from_slice(&u16::from(self.scheme_id).to_le_bytes());\n",
            "        // MUTANT: scheme id intentionally omitted.\n",
        )],
        AGGREGATE_COMMAND,
        "aggregate_root_binds_domain_scheme_and_child_root",
    ),
    (
        "proof decoder accepts bitmap/sibling count mismatch",
        PROOF,
        [(
            "        if encoded_siblings != expected_siblings || encoded_siblings > MAX_SMT_SIBLINGS {\n",
            "        if encoded_siblings > MAX_SMT_SIBLINGS {\n",
        )],
        STATE_COMMAND,
        "proof_decoder_rejects_malformed_and_redundant_default_siblings",
    ),
    (
        "missing persisted non-empty node is treated as empty state",
        SOURCE,
        [(
            "    let node = source\n"
            "        .get_node(&requested_hash)?\n"
            "        .ok_or(StateError::MissingNode(requested_hash))?;\n",
            "    let node = match source.get_node(&requested_hash)? {\n"
            "        Some(node) => node,\n"
            "        None => {\n"
            "            let empty = crate::empty_hashes(domain);\n"
            "            return Ok(StateNode::Branch {\n"
            "                depth: depth as u16,\n"
            "                left: empty[depth + 1],\n"
            "                right: empty[depth + 1],\n"
            "            });\n"
            "        }\n"
            "    };\n",
        )],
        STATE_COMMAND,
        "missing_nonempty_node_is_corruption_not_empty_state",
    ),
    (
        "proof construction explicitly retains default siblings",
        PROOF,
        [(
            "        if sibling != empty[depth + 1] {\n",
            "        if true || sibling != empty[depth + 1] {\n",
        )],
        STATE_COMMAND,
        "literal_single_membership_and_empty_nonmembership_proofs_are_canonical",
    ),
]


def run(command):
    return subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=240,
        check=False,
    )


def assert_clean(label, command):
    result = run(command)
    if result.returncode != 0 or "test result: ok." not in result.stdout:
        raise SystemExit(f"Clean {label} baseline failed:\n{result.stdout}")


def main():
    originals = {
        path: path.read_bytes()
        for path in {mutation[1] for mutation in MUTATIONS}
    }

    assert_clean("contract-state", STATE_COMMAND)
    assert_clean("state-commitment", AGGREGATE_COMMAND)

    for name, path, edits, command, test in MUTATIONS:
        source = originals[path].decode("utf-8")
        for old, new in edits:
            if source.count(old) != 1:
                raise SystemExit(f"Mutation site changed: {name}; review the mutation gate")
            source = source.replace(old, new, 1)
        try:
            path.write_text(source)
            result = run(command + [test, "--", "--exact"])
            if (
                result.returncode != 101
                or f"test {test} ... FAILED" not in result.stdout
                or "error[E" in result.stdout
            ):
                raise SystemExit(
                    f"Mutation not killed by intended test: {name}\n{result.stdout}"
                )
            print(f"KILLED: {name} — {test}", flush=True)
        finally:
            path.write_bytes(originals[path])

    assert_clean("restored contract-state", STATE_COMMAND)
    assert_clean("restored state-commitment", AGGREGATE_COMMAND)
    print(
        f"{len(MUTATIONS)}/{len(MUTATIONS)} mutations killed; restored clean suites passed",
        flush=True,
    )


if __name__ == "__main__":
    main()
