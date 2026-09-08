"""Regression checks using captured Kani 0.67.0 output, not proof execution."""
import json
from pathlib import Path
import unittest
import sys
import tempfile
import os
from unittest.mock import patch

from verify_reserve_proofs import GateError, parse_bootstrap, run, proof_environment

FIXTURES = Path(__file__).resolve().parents[1] / 'verification/reserve-conservation/evidence/bootstrap'


def output(name):
    return json.loads((FIXTURES / (name + '.json')).read_text())['output']


class ParserTests(unittest.TestCase):
    def test_reject_conflicting_or_repeated_backend_inventory(self):
        for name, status in (('positive', 0), ('negative', 1)):
            original = output(name)
            variants = [
                'CBMC 6.9.0 (cbmc-6.9.0)\n' + original,
                'CBMC 6.8.0 (cbmc-6.8.0)\n' + original,
                original.replace('CBMC version 6.8.0', 'CBMC version 6.9.0'),
                original.replace('CBMC version 6.8.0 (cbmc-6.8.0) 64-bit x86_64 linux\n', ''),
            ]
            for corrupted in variants:
                with self.subTest(name=name, log=corrupted[:200]):
                    with self.assertRaises(GateError):
                        parse_bootstrap(corrupted, status, 'bootstrap_' + name)

    def test_reject_property_blocks_outside_results(self):
        rogue = ('Check 99: unrelated.assertion.1\n'
                 ' - Status: FAILURE\n'
                 ' - Description: unrelated assertion\n'
                 ' - Location: other.rs:1:1 in function unrelated\n')
        for name, status in (('positive', 0), ('negative', 1)):
            original = output(name)
            for corrupted in (rogue + original, original.replace('SUMMARY:', 'SUMMARY:\n' + rogue)):
                with self.subTest(name=name, log=corrupted[:100]):
                    with self.assertRaises(GateError):
                        parse_bootstrap(corrupted, status, 'bootstrap_' + name)

    def test_reject_conflicting_completion_before_valid_suffix(self):
        for name, status in (('positive', 0), ('negative', 1)):
            corrupted = output(name).replace(
                'Manual Harness Summary:\n',
                'Manual Harness Summary:\nComplete - 0 successfully verified harnesses, 0 failures, 0 total.\n')
            with self.subTest(name=name):
                with self.assertRaises(GateError):
                    parse_bootstrap(corrupted, status, 'bootstrap_' + name)

    def test_real_positive_and_rerun(self):
        for name in ('positive', 'positive_after_negative'):
            self.assertEqual(len(parse_bootstrap(output(name), 0, 'bootstrap_positive')), 3)

    def test_real_ci_infrastructure_failure_is_not_a_kill(self):
        fixture = json.loads((FIXTURES.parent / 'runner/missing-backend.json').read_text())
        for harness in ('bootstrap_positive', 'bootstrap_negative'):
            with self.assertRaises(GateError):
                parse_bootstrap(fixture['output'], fixture['returncode'], harness)

    def test_real_negative(self):
        self.assertEqual(len(parse_bootstrap(output('negative'), 1, 'bootstrap_negative')), 1)

    def test_reject_false_green_and_false_kills(self):
        positive = output('positive')
        negative = output('negative')
        cases = [
            ('', 0, 'bootstrap_positive'),
            ('error: compilation failed', 1, 'bootstrap_negative'),
            (negative, 0, 'bootstrap_negative'),
            (negative, -9, 'bootstrap_negative'),
            (positive, 1, 'bootstrap_positive'),
            (negative.replace('vec![255]', 'vec![254]'), 1, 'bootstrap_negative'),
            (negative.replace('assertion.1', 'unwind.1'), 1, 'bootstrap_negative'),
            (negative.replace('Status: FAILURE', 'Status: UNKNOWN'), 1, 'bootstrap_negative'),
            (negative.replace('wrapping increment must produce a counterexample', 'unrelated failure'), 1, 'bootstrap_negative'),
            (positive.replace('Status: SATISFIED', 'Status: UNSATISFIABLE'), 0, 'bootstrap_positive'),
            (positive.replace('Status: SUCCESS', 'Status: FAILURE', 1), 0, 'bootstrap_positive'),
            (positive.replace('Kani Rust Verifier 0.67.0', 'Kani Rust Verifier 0.68.0'), 0, 'bootstrap_positive'),
            (positive.replace('CaDiCaL 2.0.0', 'MiniSat'), 0, 'bootstrap_positive'),
            (positive.replace('Check 3:', 'Check 4:'), 0, 'bootstrap_positive'),
            (positive + positive, 0, 'bootstrap_positive'),
            (positive.split('SUMMARY:')[0], 0, 'bootstrap_positive'),
            (positive + '\nerror: unsupported operation\n', 0, 'bootstrap_positive'),
            (positive.replace('0 of 2 failed', '0 of 0 failed'), 0, 'bootstrap_positive'),
            (positive, 0, 'unexpected_harness'),
        ]
        for log, status, harness in cases:
            with self.subTest(status=status, harness=harness, log=log[-100:]):
                with self.assertRaises(GateError):
                    parse_bootstrap(log, status, harness)


class ProcessTests(unittest.TestCase):
    def test_pinned_backend_is_available_to_driver_children(self):
        with tempfile.TemporaryDirectory() as directory:
            home = Path(directory)
            (home / 'bin').mkdir()
            backend = home / 'bin/goto-cc'
            backend.write_text('#!/bin/sh\necho pinned-backend\n')
            backend.chmod(0o755)
            with patch.dict(os.environ, {'PATH': '/usr/bin',
                                         'LD_LIBRARY_PATH': '/x/toolchains/wrong/lib:/usr/lib'}):
                env = proof_environment(home, 'nightly-2025-11-21')
                result = run(['goto-cc'], directory, 1, env=env)
            self.assertEqual(result['output'].strip(), 'pinned-backend')
            self.assertEqual(result['returncode'], 0)
            self.assertEqual(env['RUSTUP_TOOLCHAIN'], 'nightly-2025-11-21')
            self.assertEqual(env['LD_LIBRARY_PATH'], '/usr/lib')

    def test_timeout_is_not_a_control_kill(self):
        result = run([sys.executable, '-c', 'import time; print("started", flush=True); time.sleep(10)'],
                     FIXTURES, 0.2)
        self.assertTrue(result['timed_out'])
        self.assertNotEqual(result['returncode'], 0)
        self.assertIn('started', result['output'])

    def test_missing_tool_rejects(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(FileNotFoundError):
                run([str(Path(directory) / 'missing-kani')], directory, 1)

    def test_compiler_failure_keeps_diagnostic(self):
        result = run([sys.executable, '-c', 'import sys; print("error: compile failed"); sys.exit(1)'],
                     FIXTURES, 1)
        self.assertFalse(result['timed_out'])
        with self.assertRaises(GateError):
            parse_bootstrap(result['output'], result['returncode'], 'bootstrap_negative')


if __name__ == '__main__':
    unittest.main()
