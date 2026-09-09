"""Check retained evidence independently of the publisher's in-memory records."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from mutation_evidence import EvidenceError, canonical_json_bytes, load_manifest
from publish_mutation_evidence import ROOT, build_result, git_identity
from test_mutation_evidence_publisher import fake_run, successful_output
from verify_mutation_evidence_package import verify_package


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.output = Path(self.temp.name)
        manifest = ROOT / "verification/mutation-evidence/manifest-v1.json"
        _, specs = load_manifest(manifest, ROOT)
        (self.output / "manifest-v1.json").write_bytes(manifest.read_bytes())
        runs = []
        for spec in specs:
            log = successful_output(spec).encode()
            run = fake_run(spec)
            name = spec.id.lower() + ".log"
            (self.output / name).write_bytes(log)
            run["raw_log"] = {"filename": name, "sha256": hashlib.sha256(log).hexdigest()}
            runs.append(run)
        commit, tree = git_identity(ROOT)
        self.result = build_result(
            manifest_sha256=hashlib.sha256(manifest.read_bytes()).hexdigest(),
            commit_sha=commit, tree_sha=tree, authorities=specs, authority_runs=runs,
        )
        self.write_result()

    def write_result(self):
        (self.output / "result-v1.json").write_bytes(canonical_json_bytes(self.result))

    def verify(self):
        return verify_package(self.output, ROOT)

    def test_complete_package(self):
        self.verify()

    def test_corrupted_log(self):
        (self.output / "ea.log").write_text("corrupt")
        with self.assertRaises(EvidenceError):
            self.verify()

    def test_forged_log_with_updated_digest(self):
        log = b"3/3 mutations killed; restored clean suite passed\n"
        (self.output / "ea.log").write_bytes(log)
        self.result["authorities"][0]["raw_log"]["sha256"] = hashlib.sha256(log).hexdigest()
        self.write_result()
        with self.assertRaises(EvidenceError):
            self.verify()

    def test_wrong_source(self):
        self.result["source"]["commit"] = "0" * 40
        self.write_result()
        with self.assertRaises(EvidenceError):
            self.verify()

    def test_missing_manifest(self):
        (self.output / "manifest-v1.json").unlink()
        with self.assertRaises(EvidenceError):
            self.verify()

    def test_mismatched_semantic_result(self):
        self.result["authorities"][0]["mutations"][0]["killing_test"] = "wrong_test"
        self.write_result()
        with self.assertRaises(EvidenceError):
            self.verify()

    def test_unexpected_file(self):
        (self.output / "extra.log").write_text("extra")
        with self.assertRaises(EvidenceError):
            self.verify()

    def test_symlinked_log(self):
        (self.output / "ea.log").unlink()
        (self.output / "ea.log").symlink_to(self.output / "ee.log")
        with self.assertRaises(EvidenceError):
            self.verify()


if __name__ == "__main__":
    unittest.main()
