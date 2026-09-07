"""Regression checks using captured Kani 0.67.0 output, not proof execution."""
import json
from pathlib import Path
import unittest
import sys
import tempfile

from verify_reserve_proofs import GateError, parse_bootstrap, run

FIXTURES = Path(__file__).resolve().parents[1] / 'verification/reserve-conservation/evidence/bootstrap'


def output(name):
    return json.loads((FIXTURES / (name + '.json')).read_text())['output']


class ParserTests(unittest.TestCase):
    def test_real_positive_and_rerun(self):
        for name in ('positive', 'positive_after_negative'):
            self.assertEqual(len(parse_bootstrap(output(name), 0, 'bootstrap_positive')), 3)

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
