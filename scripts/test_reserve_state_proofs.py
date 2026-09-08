"""Semantic RED/green gates for the RC03/04/05/06/07/10 state proof slice."""
from pathlib import Path
import re
import unittest

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


if __name__ == '__main__':
    unittest.main()
