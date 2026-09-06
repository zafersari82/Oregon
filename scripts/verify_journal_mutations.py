#!/usr/bin/env python3
"""Kill the required Stage 4A bounded journal security mutations."""

from hashlib import sha256
from pathlib import Path
import json
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
COMMIT = ROOT / "crates/oregon-execution/src/journal/commit.rs"
CONSTRUCT = ROOT / "crates/oregon-execution/src/journal/construct.rs"
FINALIZE = ROOT / "crates/oregon-execution/src/journal/finalize.rs"
IO = ROOT / "crates/oregon-execution/src/journal/io.rs"
LIFECYCLE = ROOT / "crates/oregon-execution/src/journal/lifecycle.rs"
WRITE = ROOT / "crates/oregon-execution/src/journal/write.rs"
VECTOR_CORPUS = ROOT / "tests/vectors/journal-v1.json"


def run(command, cwd=ROOT):
    return subprocess.run(
        command,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )


def cargo_test(test_target, test_name=None, cwd=ROOT):
    command = [
        "cargo",
        "+1.85.0",
        "test",
        "--locked",
        "-p",
        "oregon-execution",
        "--test",
        test_target,
    ]
    if test_name is not None:
        command.extend([test_name, "--", "--exact"])
    return run(command, cwd)


def require_clean():
    result = run(["git", "status", "--porcelain", "--untracked-files=all"])
    if result.returncode != 0 or result.stdout.strip():
        raise SystemExit("journal mutation gate requires a clean disposable checkout")


def require_baseline(cwd=ROOT):
    generator = run(["python3", "scripts/generate_journal_vectors.py", "--check"], cwd)
    if generator.returncode != 0:
        print(generator.stdout)
        raise SystemExit("clean Stage 4A independent-vector baseline failed")

    for target in ["journal", "journal_security", "journal_vectors"]:
        result = cargo_test(target, cwd=cwd)
        if result.returncode != 0:
            print(result.stdout)
            raise SystemExit(f"clean Stage 4A test baseline failed: {target}")


def require_vector_negative_control():
    """A changed expected root must be rejected by both oracle and Rust consumer."""
    with tempfile.TemporaryDirectory(prefix="oregon-journal-vector-negative-") as directory:
        disposable = Path(directory) / "repo"
        shutil.copytree(
            ROOT,
            disposable,
            ignore=shutil.ignore_patterns("target", ".git", ".worktrees"),
        )
        corpus = disposable / VECTOR_CORPUS.relative_to(ROOT)
        original = corpus.read_text()
        original_hash = sha256(corpus.read_bytes()).hexdigest()
        document = json.loads(original)
        old_root = document["cases"][0]["expected_domains"][0]["new_root_hex"]
        replacement = ("0" if old_root[0] != "0" else "1") + old_root[1:]
        document["cases"][0]["expected_domains"][0]["new_root_hex"] = replacement
        corpus.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")

        try:
            generator = run(
                ["python3", "scripts/generate_journal_vectors.py", "--check"],
                disposable,
            )
            if generator.returncode == 0:
                raise SystemExit("independent journal oracle accepted a changed expected root")

            result = cargo_test("journal_vectors", cwd=disposable)
            expected_failure = (
                result.returncode == 101
                and "test independent_journal_vectors_match_runtime_trace_results ... FAILED"
                in result.stdout
                and "could not compile" not in result.stdout
            )
            if not expected_failure:
                print(result.stdout)
                raise SystemExit("Rust journal vector consumer accepted a changed expected root")
            print("REJECTED: changed expected journal root [oracle + Rust consumer]")
        finally:
            corpus.write_text(original)

        restored_hash = sha256(corpus.read_bytes()).hexdigest()
        if restored_hash != original_hash:
            raise SystemExit("journal vector negative-control restoration failed")


MUTATIONS = [
    (
        "child_revert_leaks",
        LIFECYCLE,
        """        self.frames.pop();
        self.live_entries = next_live_entries;
        self.retained_bytes = next_retained_bytes;
""",
        """        let child = self
            .frames
            .pop()
            .ok_or(JournalError::AccountingInvariant)?;
        let parent = self
            .frames
            .last_mut()
            .ok_or(JournalError::AccountingInvariant)?;
        for (domain, writes) in child.writes {
            parent.writes.entry(domain).or_default().extend(writes);
        }
        self.live_entries = next_live_entries;
        self.retained_bytes = next_retained_bytes;
""",
        "journal",
        "nested_commit_remains_revertible_by_its_parent",
    ),
    (
        "ancestor_revert_keeps_child",
        COMMIT,
        """        let parent = self
            .frames
            .last_mut()
            .ok_or(JournalError::AccountingInvariant)?;
""",
        """        let parent = self
            .frames
            .first_mut()
            .ok_or(JournalError::AccountingInvariant)?;
""",
        "journal",
        "nested_commit_remains_revertible_by_its_parent",
    ),
    (
        "delete_falls_through",
        IO,
        "                JournalValue::Delete => None,\n",
        "                JournalValue::Delete => read_value(self.source, snapshot, key)?,\n",
        "journal_security",
        "delete_shadows_persisted_base_value",
    ),
    (
        "empty_value_becomes_absent",
        IO,
        "                JournalValue::Put(bytes) => Some(bytes.clone()),\n",
        """                JournalValue::Put(bytes) => {
                    if bytes.is_empty() { None } else { Some(bytes.clone()) }
                }
""",
        "journal",
        "deletion_and_present_empty_are_distinct_and_last_write_wins",
    ),
    (
        "wrong_domain_allowed",
        CONSTRUCT,
        """            CommitmentDomainId::NativeUtxo | CommitmentDomainId::Evm => {
                Err(JournalError::UnsupportedDomain(domain))
            }
""",
        """            CommitmentDomainId::NativeUtxo => {
                Err(JournalError::UnsupportedDomain(domain))
            }
            CommitmentDomainId::Evm => Ok(()),
""",
        "journal",
        "constructor_rejects_invalid_snapshot_sets_before_use",
    ),
    (
        "frame_depth_bypass",
        LIFECYCLE,
        "        if next_depth > self.limits.max_depth {\n",
        "        if false && next_depth > self.limits.max_depth {\n",
        "journal",
        "structural_limits_are_exact_and_frame_count_is_not_refunded",
    ),
    (
        "frame_count_refunded",
        LIFECYCLE,
        """        self.frames.pop();
        self.live_entries = next_live_entries;
        self.retained_bytes = next_retained_bytes;
""",
        """        self.frames.pop();
        self.total_frames_created = self.total_frames_created.saturating_sub(1);
        self.live_entries = next_live_entries;
        self.retained_bytes = next_retained_bytes;
""",
        "journal",
        "structural_limits_are_exact_and_frame_count_is_not_refunded",
    ),
    (
        "retained_bytes_undercounted",
        WRITE,
        "            .checked_add(value.map_or(0, |bytes| bytes.len()))\n",
        "            .checked_add(0)\n",
        "journal",
        "retained_entry_and_byte_limits_count_each_live_frame_copy",
    ),
    (
        "entry_limit_bypass",
        WRITE,
        "        if next_live_entries > self.limits.max_entries {\n",
        "        if false && next_live_entries > self.limits.max_entries {\n",
        "journal",
        "retained_entry_and_byte_limits_count_each_live_frame_copy",
    ),
    (
        "open_child_finalize_allowed",
        FINALIZE,
        "        if self.frames.len() != 1 {\n",
        "        if false && self.frames.len() != 1 {\n",
        "journal",
        "root_lifecycle_and_unconfigured_domains_fail_closed",
    ),
    (
        "partial_domain_result_published",
        FINALIZE,
        "            let transition = apply_write_set(source, snapshot, &write_set)?;\n",
        """            let Ok(transition) = apply_write_set(source, snapshot, &write_set) else {
                continue;
            };
""",
        "journal",
        "later_domain_failure_returns_no_partial_bundle_and_mutates_no_source",
    ),
    (
        "unchecked_base_read",
        IO,
        "        Ok(read_value(self.source, snapshot, key)?)\n",
        "        Ok(None)\n",
        "journal_security",
        "base_reads_reject_corrupt_authoritative_values",
    ),
]


def main():
    require_clean()
    require_baseline()
    require_vector_negative_control()
    killed = 0

    with tempfile.TemporaryDirectory(prefix="oregon-journal-mutants-") as directory:
        disposable = Path(directory) / "repo"
        shutil.copytree(
            ROOT,
            disposable,
            ignore=shutil.ignore_patterns("target", ".git", ".worktrees"),
        )
        run(["git", "init", "-q"], disposable)

        for name, source_path, old, new, target, test_name in MUTATIONS:
            source = disposable / source_path.relative_to(ROOT)
            original = source.read_text()
            original_hash = sha256(source.read_bytes()).hexdigest()
            if original.count(old) != 1:
                raise SystemExit(f"mutation site is not unique: {name}")

            source.write_text(original.replace(old, new, 1))
            try:
                result = cargo_test(target, test_name, disposable)
                expected_failure = (
                    result.returncode == 101
                    and f"test {test_name} ... FAILED" in result.stdout
                    and "could not compile" not in result.stdout
                )
                if not expected_failure:
                    print(result.stdout)
                    raise SystemExit(f"mutant survived or failed to compile: {name}")
                killed += 1
                print(f"KILLED: {name} [{test_name}]")
            finally:
                source.write_text(original)

            restored_hash = sha256(source.read_bytes()).hexdigest()
            if restored_hash != original_hash:
                raise SystemExit(f"source hash restoration failed: {name}")

    require_baseline()
    require_clean()
    print(f"Stage 4A journal mutations: {killed}/{len(MUTATIONS)} killed")


if __name__ == "__main__":
    main()
