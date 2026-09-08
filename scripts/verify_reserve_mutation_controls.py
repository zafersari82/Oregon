"""Fail-closed disposable-model mutation controls for Oregon reserve proofs."""
import argparse
import json
from pathlib import Path
import re
import shutil
import tempfile

from verify_reserve_proofs import (
    GateError,
    PROOF,
    PROOF_HARNESSES,
    ROOT,
    _parse_properties,
    _reject_infrastructure_diagnostics,
    _require_common_kani_identity,
    _run_harness,
    checked,
    digest,
    parse_successful_proof,
    preflight,
    require,
)

CONTROL_CASES = (
    {
        'id': 'control_rc01_wrapping_add',
        'obligation': 'RC01',
        'harness': 'rc01_arithmetic_matches_integer_equation',
        'expected_failures': (
            'RC01 accepted arithmetic matches the independent integer equation',
        ),
        'old': '''    let after_deposit = previous
        .checked_add(input.native_deposit_total)
        .ok_or(ArithmeticError::ArithmeticOverflow)?;
''',
        'new': '''    let after_deposit = previous.wrapping_add(input.native_deposit_total);
''',
    },
    {
        'id': 'control_rc02_missing_execution_equality',
        'obligation': 'RC02',
        'harness': 'rc02_result_matches_claimed_execution_total',
        'expected_failures': (
            'RC02 accepted reserve equals the claimed execution total',
        ),
        'old': '''    if result != input.new_execution_balance_total {
        return Err(ArithmeticError::ExecutionBalanceMismatch);
    }

''',
        'new': '''    // Mutation control: execution-total equality rejection intentionally removed.

''',
    },
    {
        'id': 'control_rc03_retain_zero_reserve',
        'obligation': 'RC03',
        'harness': 'rc03_zero_and_positive_reserve_cardinality',
        'expected_failures': (
            'RC03 accepted zero result has no reserve and positive result has exactly one',
        ),
        'old': '''    if let Some(index) = previous_index {
        overlay.slots[index] = None;
    }

''',
        'new': '''    if transition.new_amount != 0 {
        if let Some(index) = previous_index {
            overlay.slots[index] = None;
        }
    }

''',
    },
    {
        'id': 'control_rc04_remove_multiple_reserve_rejection',
        'obligation': 'RC04',
        'harness': 'rc04_multiple_live_reserves_reject_unchanged',
        'expected_failures': (
            'RC04 two live reserves reject with the multiple-reserve error',
            'RC04 two-live-reserve rejection leaves the full state unchanged',
        ),
        'old': '''    if reserve_indices.iter().flatten().count() > 1 {
        return Err(StateError::MultipleLiveReserves);
    }

''',
        'new': '''    // Mutation control: multiple-live-reserve rejection intentionally removed.

''',
    },
    {
        'id': 'control_rc05_create_ordinary_program_reserve',
        'obligation': 'RC05',
        'harness': 'rc05_created_reserve_exact_entry',
        'expected_failures': (
            'RC05 undo records the exact created reserve entry',
            'RC05 successful creation has exact reserve program value and metadata',
        ),
        'old': '''                program: ProgramClass::Reserve,
''',
        'new': '''                program: ProgramClass::Ordinary,
''',
    },
    {
        'id': 'control_rc06_remove_previous_before_collision',
        'obligation': 'RC06',
        'harness': 'rc06_failed_apply_and_undo_are_atomic',
        'expected_failures': (
            'RC06 failed apply leaves every modeled entry unchanged',
        ),
        'old': '''    if let Some(new_key) = transition.new_key {
        if state.slots.iter().flatten().any(|slot| slot.key == new_key) {
            return Err(StateError::OutputCollision);
        }
    }

''',
        'new': '''    if let Some(new_key) = transition.new_key {
        if state.slots.iter().flatten().any(|slot| slot.key == new_key) {
            if let Some(index) = previous_index {
                state.slots[index] = None;
            }
            return Err(StateError::OutputCollision);
        }
    }

''',
    },
    {
        'id': 'control_rc07_omit_previous_restoration',
        'obligation': 'RC07',
        'harness': 'rc07_apply_then_undo_restores_full_state',
        'expected_failures': (
            'RC07 apply followed by untampered undo restores the complete pre-state',
        ),
        'old': '''    if let Some(previous) = undo.previous {
        let Some(index) = overlay.slots.iter().position(Option::is_none) else {
            return Err(StateError::UndoMismatch);
        };
        overlay.slots[index] = Some(previous);
    }

''',
        'new': '''    // Mutation control: previous-entry restoration intentionally omitted.

''',
    },
    {
        'id': 'control_rc08_omit_fee_subtraction',
        'obligation': 'RC08',
        'harness': 'rc08_conservation_identity',
        'expected_failures': (
            'RC08 accepted transition conserves reserve arithmetic as mathematical integers',
        ),
        'old': '''    let result = after_withdrawal
        .checked_sub(input.execution_fee_total)
        .ok_or(ArithmeticError::ArithmeticUnderflow)?;
''',
        'new': '''    let result = after_withdrawal;
''',
    },
    {
        'id': 'control_rc09_saturating_subtraction',
        'obligation': 'RC09',
        'harness': 'rc09_invalid_arithmetic_and_endpoints_reject',
        'expected_failures': (
            'RC09 withdrawal underflow rejects',
        ),
        'old': '''    let after_withdrawal = after_deposit
        .checked_sub(input.execution_withdrawal_total)
        .ok_or(ArithmeticError::ArithmeticUnderflow)?;
''',
        'new': '''    let after_withdrawal = after_deposit.saturating_sub(input.execution_withdrawal_total);
''',
    },
    {
        'id': 'control_rc10_skip_created_entry_equality',
        'obligation': 'RC10',
        'harness': 'rc10_collisions_and_tampered_undo_reject_unchanged',
        'expected_failures': (
            'RC10 stale or tampered created-entry undo rejects',
            'RC10 stale or tampered undo rejection leaves the full state unchanged',
        ),
        'old': '''            if actual != expected {
                return Err(StateError::UndoMismatch);
            }
''',
        'new': '''            // Mutation control: exact created-entry equality validation omitted.
''',
    },
)


def _description(match):
    return re.sub(r'\s+', ' ', match[4].strip()).strip('"')


def parse_negative_control(output, returncode, control_id):
    """Accept a control only when exactly its selected semantic assertions are killed."""
    case = next((item for item in CONTROL_CASES if item['id'] == control_id), None)
    require(case is not None, 'unknown mutation control')
    harness = case['harness']
    expected_failures = list(case['expected_failures'])
    require(returncode == 1, 'mutation control did not fail verification')
    _reject_infrastructure_diagnostics(output)
    _require_common_kani_identity(output, harness)
    matches = _parse_properties(output)

    failures = [match for match in matches if match[3] == 'FAILURE']
    require(failures, 'mutation control has no failing property')
    require(
        [_description(match) for match in failures] == expected_failures,
        'mutation control did not kill exactly the selected semantic assertions',
    )
    for failed in failures:
        require(
            failed[2].startswith(harness + '.assertion.'),
            'mutation control failed outside its selected proof assertion',
        )
        require(
            failed[5].endswith('in function ' + harness),
            'mutation failure is not owned by selected proof harness',
        )

    for match in matches:
        if match in failures:
            continue
        property_name = match[2]
        status = match[3]
        if property_name.startswith(harness + '.assertion.'):
            require(status == 'SUCCESS', 'unexpected selected assertion status')
        elif property_name.startswith(harness + '.cover.'):
            require(
                status in {'SATISFIED', 'UNSATISFIABLE'},
                'unexpected mutation-control cover status',
            )
        else:
            require(
                status in {'SUCCESS', 'UNREACHABLE'},
                'unexpected internal mutation-control property failure',
            )

    summaries = re.findall(
        r'^ \*\* (\d+) of (\d+) failed(?: \(\d+ unreachable\))?$', output, re.M
    )
    require(
        len(summaries) == 1 and int(summaries[0][0]) == len(expected_failures),
        'wrong mutation failure summary',
    )
    require(re.findall(r'^VERIFICATION:- (.+)$', output, re.M) == ['FAILED'], 'wrong control verdict')
    require(
        'let concrete_vals: Vec<Vec<u8>> = vec![' in output,
        'mutation control has no concrete counterexample',
    )
    require(
        'kani::concrete_playback_run(concrete_vals, ' + harness + ');' in output,
        'counterexample is not bound to selected proof harness',
    )
    require(
        output.rstrip().endswith(
            'Complete - 0 successfully verified harnesses, 1 failures, 1 total.'
        ),
        'incomplete mutation-control harness summary',
    )
    return [
        {
            'property': match[2],
            'status': match[3],
            'description': _description(match),
        }
        for match in matches
    ]


def control_preflight(kani_home, archive, rustc):
    lock = preflight(kani_home, archive, rustc, 'all')
    baseline = (PROOF / 'src/model.rs').read_text()
    control_harnesses = [case['harness'] for case in CONTROL_CASES]
    require(
        len(control_harnesses) == len(PROOF_HARNESSES)
        and len(set(control_harnesses)) == len(control_harnesses)
        and set(control_harnesses) == set(PROOF_HARNESSES),
        'unexpected mutation-control harness mapping',
    )
    for case in CONTROL_CASES:
        require(
            baseline.count(case['old']) == 1,
            'mutation anchor missing or duplicated: ' + case['id'],
        )
        require(case['expected_failures'], 'mutation control has no expected semantic kill')
    return lock


def mutated_checkout(case):
    """Create a disposable proof source tree containing one exact model mutation."""
    temporary = tempfile.TemporaryDirectory(prefix=case['id'] + '-')
    root = Path(temporary.name)
    (root / 'src').mkdir()
    shutil.copy2(PROOF / 'proofs.rs', root / 'proofs.rs')
    baseline = (PROOF / 'src/model.rs').read_text()
    require(baseline.count(case['old']) == 1, 'mutation anchor changed during execution')
    (root / 'src/model.rs').write_text(baseline.replace(case['old'], case['new'], 1))
    return temporary, root


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--kani-home', type=Path, required=True)
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--rustc', type=Path, required=True)
    parser.add_argument('--output', required=True, type=Path, help='new evidence directory')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)

    evidence = {
        'schema_version': 1,
        'scope': 'reserve-conservation-mutation-controls',
        'accepted': False,
        'mutation_controls_executed': [],
        'baseline_positive_rerun': [],
        'runs': [],
    }
    try:
        evidence['source_sha'] = checked(['git', 'rev-parse', 'HEAD'], ROOT)
        evidence['source_tree'] = checked(['git', 'rev-parse', 'HEAD^{tree}'], ROOT)
        evidence['worktree_status'] = checked(['git', 'status', '--porcelain'], ROOT)
        require(not evidence['worktree_status'], 'mutation evidence requires a clean source checkout')
        kani_home, archive, rustc = (
            path.resolve() for path in (args.kani_home, args.archive, args.rustc)
        )
        evidence['tool_lock'] = control_preflight(kani_home, archive, rustc)
        baseline_model = PROOF / 'src/model.rs'
        baseline_digest = digest(baseline_model)
        sources = [
            PROOF / 'proofs.rs',
            baseline_model,
            PROOF / 'toolchain-lock.json',
            Path(__file__).resolve(),
            ROOT / 'scripts/verify_reserve_proofs.py',
        ]
        evidence['source_digests'] = {
            str(path.relative_to(ROOT)): digest(path) for path in sources
        }

        for case in CONTROL_CASES:
            temporary, mutated_root = mutated_checkout(case)
            try:
                mutated_model = mutated_root / 'src/model.rs'
                result = _run_harness(
                    kani_home,
                    mutated_root / 'proofs.rs',
                    case['harness'],
                    evidence['tool_lock'],
                    timeout=300,
                    concrete=True,
                )
                result['control_id'] = case['id']
                result['obligation'] = case['obligation']
                result['mutated_model_sha256'] = digest(mutated_model)
                evidence['runs'].append(result)
                require(not result['timed_out'], 'mutation verifier timed out: ' + case['id'])
                result['properties'] = parse_negative_control(
                    result['output'], result['returncode'], case['id']
                )
                evidence['mutation_controls_executed'].append(case['id'])
            finally:
                temporary.cleanup()
            require(digest(baseline_model) == baseline_digest, 'baseline model changed by control')
            require(
                not checked(['git', 'status', '--porcelain'], ROOT),
                'repository changed by disposable mutation control',
            )

        require(
            evidence['mutation_controls_executed'] == [case['id'] for case in CONTROL_CASES],
            'incomplete mutation-control execution',
        )

        # The spec requires restoration verification and a fresh complete positive pass.
        for harness in PROOF_HARNESSES:
            result = _run_harness(
                kani_home,
                PROOF / 'proofs.rs',
                harness,
                evidence['tool_lock'],
                timeout=300,
            )
            require(not result['timed_out'], 'baseline positive rerun timed out: ' + harness)
            properties = parse_successful_proof(result['output'], result['returncode'], harness)
            evidence['baseline_positive_rerun'].append(
                {'harness': harness, 'properties': properties}
            )
        require(
            [item['harness'] for item in evidence['baseline_positive_rerun']]
            == list(PROOF_HARNESSES),
            'incomplete baseline positive rerun',
        )
        require(digest(baseline_model) == baseline_digest, 'baseline digest changed after controls')
        require(not checked(['git', 'status', '--porcelain'], ROOT), 'final baseline checkout is dirty')
        evidence['accepted'] = True
    except (GateError, OSError, ValueError) as error:
        evidence['error'] = str(error)

    (args.output / 'summary.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(
        json.dumps(
            {
                'accepted': evidence['accepted'],
                'scope': evidence['scope'],
                'error': evidence.get('error'),
            }
        )
    )
    return 0 if evidence['accepted'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
