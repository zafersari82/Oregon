"""Semantic RED/green gates for the RC03/04/05/06/07/10 state proof slice."""
from pathlib import Path
import re
import unittest
from unittest.mock import patch

import verify_reserve_proofs as verifier

ROOT = Path(__file__).resolve().parents[1]
PROOFS = ROOT / 'verification/reserve-conservation/proofs.rs'

EXPECTED_STATE_HARNESSES = [
    'rc03_zero_and_positive_reserve_cardinality',
    'rc04_multiple_live_reserves_reject_unchanged',
    'rc05_created_reserve_exact_entry',
    'rc06_failed_apply_and_undo_are_atomic',
    'rc07_apply_then_undo_restores_full_state',
    'rc10_collisions_and_tampered_undo_reject_unchanged',
]


class StateProofInventoryTests(unittest.TestCase):
    def test_state_proof_harness_inventory_is_explicit(self):
        source = PROOFS.read_text()
        actual = re.findall(
            r'^fn (rc(?:03|04|05|06|07|10)_[a-z0-9_]+)\(', source, re.M
        )
        self.assertEqual(actual, EXPECTED_STATE_HARNESSES)

        for harness in EXPECTED_STATE_HARNESSES:
            with self.subTest(harness=harness):
                body_start = source.index('fn ' + harness + '(')
                prefix = source[max(0, body_start - 96):body_start]
                self.assertIn('#[kani::proof]', prefix)
                self.assertIn('#[kani::solver(cadical)]', prefix)

    def test_runner_exposes_the_exact_state_harness_inventory(self):
        self.assertEqual(
            list(getattr(verifier, 'STATE_HARNESSES', ())),
            EXPECTED_STATE_HARNESSES,
        )

    def test_state_harnesses_receive_explicit_five_step_unwind_bound(self):
        lock = {'rust_toolchain': 'nightly-test'}
        with patch.object(verifier, 'run', return_value={'returncode': 0, 'output': '', 'timed_out': False}) as mocked:
            verifier._run_harness(
                Path('/kani-home'),
                PROOFS,
                EXPECTED_STATE_HARNESSES[0],
                lock,
                timeout=1,
            )
        command = mocked.call_args.args[0]
        self.assertEqual(command[-2:], ['--unwind', '5'])

    def test_arithmetic_harnesses_do_not_inherit_state_unwind_bound(self):
        lock = {'rust_toolchain': 'nightly-test'}
        with patch.object(verifier, 'run', return_value={'returncode': 0, 'output': '', 'timed_out': False}) as mocked:
            verifier._run_harness(
                Path('/kani-home'),
                PROOFS,
                verifier.ARITHMETIC_HARNESSES[0],
                lock,
                timeout=1,
            )
        command = mocked.call_args.args[0]
        self.assertNotIn('--unwind', command)

    def test_positive_parser_accepts_named_state_assertions_and_reachability(self):
        harness = 'rc04_multiple_live_reserves_reject_unchanged'
        output = f'''Kani Rust Verifier 0.67.0 (standalone)
Checking harness {harness}...
CBMC 6.8.0 (cbmc-6.8.0)
Solving with CaDiCaL 2.0.0
RESULTS:
Check 1: {harness}.assertion.1
 - Status: SUCCESS
 - Description: "RC04 two live reserves reject with the multiple-reserve error"
 - Location: proofs.rs:1:1 in function {harness}
Check 2: {harness}.assertion.2
 - Status: SUCCESS
 - Description: "RC04 two-live-reserve rejection leaves the full state unchanged"
 - Location: proofs.rs:2:1 in function {harness}
Check 3: {harness}.cover.1
 - Status: SATISFIED
 - Description: "RC04 multiple-live-reserve rejection is reachable"
 - Location: proofs.rs:3:1 in function {harness}
SUMMARY:
 ** 0 of 2 failed
 ** 1 of 1 cover properties satisfied
VERIFICATION:- SUCCESSFUL
Manual Harness Summary:
Complete - 1 successfully verified harnesses, 0 failures, 1 total.
'''
        properties = verifier.parse_successful_proof(output, 0, harness)
        self.assertEqual(len(properties), 3)


if __name__ == '__main__':
    unittest.main()
