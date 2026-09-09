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


def rc03_output(internal_property):
    harness = EXPECTED_STATE_HARNESSES[0]
    return f'''Kani Rust Verifier 0.67.0 (standalone)
Checking harness {harness}...
CBMC 6.8.0 (cbmc-6.8.0)
Solving with CaDiCaL 2.0.0
RESULTS:
{internal_property}Check 2: {harness}.assertion.1
 - Status: SUCCESS
 - Description: "RC03 constructed valid state transition is accepted"
 - Location: proofs.rs:1:1 in function {harness}
Check 3: {harness}.cover.1
 - Status: SATISFIED
 - Description: "RC03 accepted zero-result branch is reachable"
 - Location: proofs.rs:2:1 in function {harness}
Check 4: {harness}.cover.2
 - Status: SATISFIED
 - Description: "RC03 accepted positive-result branch is reachable"
 - Location: proofs.rs:3:1 in function {harness}
SUMMARY:
 ** 0 of 2 failed
 ** 2 of 2 cover properties satisfied
VERIFICATION:- SUCCESSFUL
Manual Harness Summary:
Complete - 1 successfully verified harnesses, 0 failures, 1 total.
'''


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

    def test_positive_parser_accepts_successful_unwinding_assertion(self):
        output = rc03_output('''Check 1: core::iter::state_loop.unwind.0
 - Status: SUCCESS
 - Description: "unwinding assertion loop 0"
 - Location: core/iter.rs:1:1 in function core::iter::state_loop
''')
        properties = verifier.parse_successful_proof(
            output, 0, EXPECTED_STATE_HARNESSES[0]
        )
        self.assertEqual(len(properties), 4)

    def test_positive_parser_accepts_successful_unsupported_safety_property(self):
        output = rc03_output('''Check 1: core::panic::caller_location.unsupported.1
 - Status: SUCCESS
 - Description: "caller_location is not currently supported by Kani"
 - Location: core/panic/location.rs:1:1 in function core::panic::caller_location
''')
        properties = verifier.parse_successful_proof(
            output, 0, EXPECTED_STATE_HARNESSES[0]
        )
        self.assertEqual(len(properties), 4)

    def test_positive_parser_accepts_exact_kani_unsupported_warning_when_property_is_safe(self):
        harness = EXPECTED_STATE_HARNESSES[0]
        output = rc03_output('''Check 1: std::panic::Location::caller.unsupported_construct.1
 - Status: SUCCESS
 - Description: "caller_location is not currently supported by Kani"
 - Location: core/panic/location.rs:147:9 in function std::panic::Location::caller
''')
        warning = '''warning: Found the following unsupported constructs:
             - caller_location (1)
         
         Verification will fail if one or more of these constructs is reachable.
         See https://model-checking.github.io/kani/rust-feature-support.html for more details.

warning: 1 warning emitted

'''
        output = output.replace(
            f'Checking harness {harness}...',
            warning + f'Checking harness {harness}...',
        )
        properties = verifier.parse_successful_proof(output, 0, harness)
        self.assertEqual(len(properties), 4)

        malformed = output.replace(
            'Verification will fail if one or more of these constructs is reachable.',
            'unsupported operation escaped proof instrumentation.',
        )
        with self.assertRaises(verifier.GateError):
            verifier.parse_successful_proof(malformed, 0, harness)

    def test_positive_parser_accepts_multiline_internal_safety_description(self):
        output = rc03_output('''Check 1: kani::rustc_intrinsics::ptr_offset_from.safety_check.2
 - Status: SUCCESS
 - Description: "Expected the distance between the pointers, in bytes, to be a
                  multiple of the size of `T`"
 - Location: kani/src/lib.rs:57:1 in function kani::rustc_intrinsics::ptr_offset_from
''')
        properties = verifier.parse_successful_proof(
            output, 0, EXPECTED_STATE_HARNESSES[0]
        )
        self.assertEqual(len(properties), 4)

    def test_positive_parser_accepts_unreachable_internal_safety_property(self):
        output = rc03_output('''Check 1: std::fmt::Arguments::from_str.assertion.1
 - Status: UNREACHABLE
 - Description: "attempt to shift left with overflow"
 - Location: core/src/fmt/mod.rs:820:38 in function std::fmt::Arguments::from_str
''')
        properties = verifier.parse_successful_proof(
            output, 0, EXPECTED_STATE_HARNESSES[0]
        )
        self.assertEqual(len(properties), 4)

    def test_positive_parser_rejects_unreachable_named_rc_assertion(self):
        harness = EXPECTED_STATE_HARNESSES[0]
        output = rc03_output('''Check 1: core::iter::state_loop.unwind.0
 - Status: SUCCESS
 - Description: "unwinding assertion loop 0"
 - Location: core/iter.rs:1:1 in function core::iter::state_loop
''').replace(
            f'Check 2: {harness}.assertion.1\n - Status: SUCCESS',
            f'Check 2: {harness}.assertion.1\n - Status: UNREACHABLE',
        )
        with self.assertRaises(verifier.GateError):
            verifier.parse_successful_proof(output, 0, harness)

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
