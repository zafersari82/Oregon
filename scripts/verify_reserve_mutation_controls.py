"""Fail-closed runner for Oregon reserve arithmetic mutation controls."""
import argparse
import json
from pathlib import Path
import re

from verify_reserve_proofs import (
    GateError,
    PROOF,
    ROOT,
    _parse_properties,
    _reject_infrastructure_diagnostics,
    _require_common_kani_identity,
    _run_harness,
    _source_harnesses,
    checked,
    digest,
    preflight,
    require,
)

CONTROL_HARNESSES = (
    'control_rc01_wrapping_add',
    'control_rc02_missing_execution_equality',
    'control_rc08_omit_fee_subtraction',
    'control_rc09_saturating_subtraction',
)
CONTROL_OBLIGATIONS = {
    'control_rc01_wrapping_add': 'RC01',
    'control_rc02_missing_execution_equality': 'RC02',
    'control_rc08_omit_fee_subtraction': 'RC08',
    'control_rc09_saturating_subtraction': 'RC09',
}


def parse_negative_control(output, returncode, harness):
    """Accept a control only when Kani kills exactly the intended RC assertion."""
    require(harness in CONTROL_HARNESSES, 'unknown mutation control harness')
    require(returncode == 1, 'mutation control did not fail verification')
    _reject_infrastructure_diagnostics(output)
    _require_common_kani_identity(output, harness)
    matches = _parse_properties(output)

    failures = [match for match in matches if match[3] == 'FAILURE']
    require(len(failures) == 1, 'mutation control must have exactly one failing property')
    require(
        all(match[3] in {'SUCCESS', 'FAILURE'} for match in matches),
        'unexpected mutation-control property status',
    )
    failed = failures[0]
    obligation = CONTROL_OBLIGATIONS[harness]
    require(
        failed[2].startswith(harness + '.assertion.'),
        'mutation control failed outside its named assertion',
    )
    require(obligation + ' control:' in failed[4].strip('"'), 'wrong mutation killed the proof')
    require(
        failed[5].endswith('in function ' + harness),
        'mutation failure is not owned by selected harness',
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
        'counterexample is not bound to selected mutation control',
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
    source = PROOF / 'mutation_controls.rs'
    text = source.read_text()
    require(
        _source_harnesses(source) == list(CONTROL_HARNESSES)
        and text.count('#[kani::proof]') == len(CONTROL_HARNESSES)
        and text.count('#[kani::solver(cadical)]') == len(CONTROL_HARNESSES),
        'unexpected mutation-control harness inventory',
    )
    return lock


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
        source = PROOF / 'mutation_controls.rs'
        sources = [
            source,
            PROOF / 'src/model.rs',
            PROOF / 'toolchain-lock.json',
            Path(__file__).resolve(),
            ROOT / 'scripts/verify_reserve_proofs.py',
        ]
        evidence['source_digests'] = {
            str(path.relative_to(ROOT)): digest(path) for path in sources
        }

        for harness in CONTROL_HARNESSES:
            result = _run_harness(
                kani_home,
                source,
                harness,
                evidence['tool_lock'],
                timeout=300,
                concrete=True,
            )
            evidence['runs'].append(result)
            require(not result['timed_out'], 'mutation verifier timed out: ' + harness)
            result['properties'] = parse_negative_control(
                result['output'], result['returncode'], harness
            )
            evidence['mutation_controls_executed'].append(harness)
        require(
            evidence['mutation_controls_executed'] == list(CONTROL_HARNESSES),
            'incomplete mutation-control execution',
        )
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
