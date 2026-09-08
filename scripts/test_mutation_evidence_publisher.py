"""Fail-closed parser and publisher tests for Mutation Evidence V1."""
import unittest

from mutation_evidence import (
    AuthoritySpec,
    EvidenceError,
    KillRecord,
    MutationSpec,
    parse_authority_output,
)


def authority(prefix="EA", count=3, syntax="dash"):
    mutations = tuple(
        MutationSpec(
            id=f"{prefix}-{index:03d}",
            name=f"mutation {index}",
            target=f"target/{index}",
            expected_broken_behavior=f"broken {index}",
            killing_test=f"test_{index}",
            kill_classification="semantic-test-failure",
        )
        for index in range(1, count + 1)
    )
    return AuthoritySpec(
        id=prefix,
        display_name=prefix,
        runner=f"scripts/{prefix.lower()}_runner.py",
        runner_sha256="0" * 64,
        command=("python3", f"scripts/{prefix.lower()}_runner.py"),
        expected_count=count,
        mutations=mutations,
    )


def successful_output(spec, syntax="dash"):
    lines = []
    for mutation in spec.mutations:
        if syntax == "dash":
            lines.append(f"KILLED: {mutation.name} — {mutation.killing_test}")
        else:
            lines.append(f"KILLED: {mutation.name} [{mutation.killing_test}]")
    if syntax == "dash":
        lines.append(
            f"{spec.expected_count}/{spec.expected_count} mutations killed; "
            "restored clean suite passed"
        )
    else:
        lines.append(
            f"Execution resource mutations: "
            f"{spec.expected_count}/{spec.expected_count} killed"
        )
    return "\n".join(lines) + "\n"


class AuthorityOutputParserTests(unittest.TestCase):
    def test_accepts_em_dash_records(self):
        spec = authority("EA", 3)
        records = parse_authority_output(spec, successful_output(spec, "dash"), 0)
        self.assertEqual(len(records), 3)
        self.assertEqual(records[0], KillRecord("EA-001", "mutation 1", "test_1"))

    def test_accepts_bracket_records(self):
        spec = authority("ER", 13)
        records = parse_authority_output(spec, successful_output(spec, "bracket"), 0)
        self.assertEqual(len(records), 13)
        self.assertEqual(records[-1].mutation_id, "ER-013")

    def assert_rejected(self, output, returncode=0, spec=None):
        with self.assertRaises(EvidenceError):
            parse_authority_output(spec or authority(), output, returncode)

    def test_duplicate_kill_record_is_rejected(self):
        spec = authority()
        output = successful_output(spec)
        duplicate = "KILLED: mutation 1 — test_1\n"
        self.assert_rejected(output.replace("KILLED: mutation 2 — test_2\n", duplicate), spec=spec)

    def test_unknown_mutation_is_rejected(self):
        spec = authority()
        output = successful_output(spec).replace("mutation 2 — test_2", "unknown — test_2")
        self.assert_rejected(output, spec=spec)

    def test_wrong_killing_test_is_rejected(self):
        spec = authority()
        output = successful_output(spec).replace("mutation 2 — test_2", "mutation 2 — wrong_test")
        self.assert_rejected(output, spec=spec)

    def test_summary_mismatch_is_rejected(self):
        spec = authority()
        output = successful_output(spec).replace("3/3 mutations killed", "2/3 mutations killed")
        self.assert_rejected(output, spec=spec)

    def test_missing_summary_is_rejected(self):
        spec = authority()
        output = "\n".join(successful_output(spec).splitlines()[:-1]) + "\n"
        self.assert_rejected(output, spec=spec)

    def test_missing_kill_is_rejected_even_with_green_summary(self):
        spec = authority()
        output = successful_output(spec).replace("KILLED: mutation 2 — test_2\n", "")
        self.assert_rejected(output, spec=spec)

    def test_nonzero_runner_exit_is_rejected(self):
        spec = authority()
        self.assert_rejected(successful_output(spec), returncode=1, spec=spec)

    def test_compiler_failure_cannot_masquerade_as_kill(self):
        spec = authority()
        for marker in ("could not compile oregon-utxo", "error[E0308]: mismatched types"):
            with self.subTest(marker=marker):
                self.assert_rejected(successful_output(spec) + marker + "\n", spec=spec)

    def test_infrastructure_failure_cannot_masquerade_as_kill(self):
        spec = authority()
        for marker in (
            "Traceback (most recent call last):",
            "process timed out",
        ):
            with self.subTest(marker=marker):
                self.assert_rejected(successful_output(spec) + marker + "\n", spec=spec)

    def test_bare_aggregate_is_never_sufficient(self):
        spec = authority()
        self.assert_rejected("3/3 mutations killed; restored clean suite passed\n", spec=spec)


if __name__ == "__main__":
    unittest.main()
