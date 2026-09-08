"""Fail-closed disposable-model mutation controls for Oregon reserve arithmetic proofs."""
import argparse
import json
from pathlib import Path
import re
import shutil
import tempfile

from verify_reserve_proofs import (
    ARITHMETIC_HARNESSES,
    GateError,
    PROOF,
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
        'old': '''    if result != input.new_execution_balance_total {
        return Err(ArithmeticError::ExecutionBalanceMismatch);
    }

''',
        'new': '''    // Mutation control: execution-total equality rejection intentionally removed.

''',
    },
    {
        'id': 'control_rc08_omit_fee_subtraction',
        'obligation': 'RC08',
        'harness': 'rc08_conservation_identity',
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
        'old': '''    let after_withdrawal = after_deposit
        .checked_sub(input.execution_withdrawal_total)
        .ok_or(ArithmeticError::ArithmeticUnderflow)?;
''',
        'new': '''    let after_withdrawal = after_deposit.saturating_sub(input.execution_withdrawal_total);
''',
    },
)


def parse_negative_control(output, returncode, control_id):
    """Accept a control only when exactly its selected RC assertion is killed."""
    case = next((item for item in CONTROL_CASES if item['id'] == control_id), None)
    require(case is not None, 'unknown mutation control')
    harness = case['harness']
    obligation = case['obligation']
    require(returncode == 1, 'mutation control did not fail verification')
    _reject_infrastructure_diagnostics(output)
    _require_common_kani_identity(output, harness)
    matches = _parse_properties(output)

    failures = [match for match in matches if match[3] == 'FAILURE']
    require(len(failures) == 1, 'mutation control must have exactly one failing property')
    failed = failures[0]
    require(
        failed[2].startswith(harness + '.assertion.'),
        'mutation control failed outside its selected proof assertion',
    )
    require(obligation in failed[4].strip('"'), 'wrong mutation killed the proof')
    require(
        failed[5].endswith('in function ' + harness),
        'mutation failure is not owned by selected proof harness',
    )
    for match in matches:
        if match is failed:
            continue
        status = match[3]
        is_cover = '.cover.' in match[2]
        require(
            status == 'SUCCESS' or (is_cover and status in {'SATISFIED', 'UNSATISFIABLE'}),
            'unexpected secondary mutation-control property failure',
        )

    summaries = re.findall(r'^ \*\* (\d+) of (\d+) failed$', output, re.M)
    require(len(summaries) == 1 and summaries[0][0] == '1', 'wrong mutation failure summary')
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
            'description': match[4].strip('"'),
        }
        for match in matches
    ]


def control_preflight(kani_home, archive, rustc):
    lock = preflight(kani_home, archive, rustc, 'arithmetic')
    baseline = (PROOF / 'src/model.rs').read_text()
    require(
        [case['harness'] for case in CONTROL_CASES]
        == [
            'rc01_arithmetic_matches_integer_equation',
            'rc02_result_matches_claimed_execution_total',
            'rc08_conservation_identity',
            'rc09_invalid_arithmetic_and_endpoints_reject',
        ],
        'unexpected mutation-control harness mapping',
    )
    for case in CONTROL_CASES:
        require(
            baseline.count(case['old']) == 1,
            'mutation anchor missing or duplicated: ' + case['id'],
        )
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
        'scope': 'reserve-arithmetic-mutation-controls',
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

        # The spec requires restoration verification and a fresh positive pass after the final kill.
        for harness in ARITHMETIC_HARNESSES:
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
            == list(ARITHMETIC_HARNESSES),
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
