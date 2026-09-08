"""Fail-closed Kani 0.67.0 verifier for Oregon reserve-conservation evidence."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
PROOF = ROOT / 'verification/reserve-conservation'

BOOTSTRAP_HARNESSES = ('bootstrap_positive', 'bootstrap_negative')
ARITHMETIC_HARNESSES = (
    'rc01_arithmetic_matches_integer_equation',
    'rc02_result_matches_claimed_execution_total',
    'rc08_conservation_identity',
    'rc09_invalid_arithmetic_and_endpoints_reject',
)
ARITHMETIC_COVER_COUNTS = {
    'rc01_arithmetic_matches_integer_equation': 2,
    'rc02_result_matches_claimed_execution_total': 2,
    'rc08_conservation_identity': 1,
    'rc09_invalid_arithmetic_and_endpoints_reject': 3,
}


class GateError(ValueError):
    """Evidence does not establish the expected verification result."""


def require(condition, message):
    if not condition:
        raise GateError(message)


def _reject_infrastructure_diagnostics(output):
    require(
        not re.search(
            r'(?im)^.*(?:error:|error\[|unsupported|unwinding assertion|timed out)',
            output,
        ),
        'infrastructure or unsupported-operation diagnostic',
    )


def _parse_properties(output):
    section = output.split('RESULTS:\n', 1)[-1].split('SUMMARY:', 1)[0]
    pattern = re.compile(
        r'Check (\d+): ([^\n]+)\n\s+- Status: ([A-Z]+)\n'
        r'\s+- Description: ([^\n]+)\n\s+- Location: ([^\n]+)\n'
    )
    matches = list(pattern.finditer(section))
    require(matches, 'empty property inventory')
    require(not pattern.sub('', section).strip(), 'unparsed property output')
    require(
        [int(match[1]) for match in matches] == list(range(1, len(matches) + 1)),
        'nonsequential property inventory',
    )
    return matches


def _require_common_kani_identity(output, harness):
    for marker in (
        'Kani Rust Verifier 0.67.0 (standalone)',
        'Checking harness ' + harness + '...',
        'RESULTS:',
        'SUMMARY:',
        'Manual Harness Summary:',
    ):
        require(output.count(marker) == 1, 'missing or repeated output marker: ' + marker)
    require(
        re.findall(r'^Checking harness (.+)\.\.\.$', output, re.M) == [harness],
        'unexpected harness inventory',
    )
    require('CBMC 6.8.0 (cbmc-6.8.0)' in output, 'wrong backend')
    solvers = re.findall(r'^Solving with (.+)$', output, re.M)
    require(solvers and set(solvers) == {'CaDiCaL 2.0.0'}, 'wrong or absent solver')


def parse_bootstrap(output, returncode, harness):
    """Accept only the observed smoke property inventory and semantic control."""
    require(harness in BOOTSTRAP_HARNESSES, 'unknown harness')
    negative = harness == 'bootstrap_negative'
    require(returncode == int(negative), 'unexpected verifier exit status')
    _reject_infrastructure_diagnostics(output)
    _require_common_kani_identity(output, harness)
    matches = _parse_properties(output)

    if negative:
        expected = [
            (
                'bootstrap_negative.assertion.1',
                'FAILURE',
                'bootstrap wrapping increment must produce a counterexample',
            )
        ]
    else:
        expected = [
            ('bootstrap_positive.assertion.1', 'SUCCESS', 'attempt to add with overflow'),
            (
                'bootstrap_positive.assertion.2',
                'SUCCESS',
                'bootstrap widening preserves increment',
            ),
            ('bootstrap_positive.cover.1', 'SATISFIED', 'maximum input is reachable'),
        ]
    actual = [(match[2], match[3], match[4].strip('"')) for match in matches]
    require(actual == expected, 'unexpected properties, statuses or descriptions')
    require(
        all(match[5].endswith('in function ' + harness) for match in matches),
        'wrong property location',
    )
    summary = '1 of 1 failed' if negative else '0 of 2 failed'
    require(
        re.findall(r'^ \*\* (\d+ of \d+ failed)$', output, re.M) == [summary],
        'inconsistent assertion summary',
    )
    verdict = 'FAILED' if negative else 'SUCCESSFUL'
    require(re.findall(r'^VERIFICATION:- (.+)$', output, re.M) == [verdict], 'wrong verdict')
    completed = (
        'Complete - 0 successfully verified harnesses, 1 failures, 1 total.'
        if negative
        else 'Complete - 1 successfully verified harnesses, 0 failures, 1 total.'
    )
    require(output.rstrip().endswith(completed), 'incomplete harness summary')
    if negative:
        require(
            re.search(
                r'let concrete_vals: Vec<Vec<u8>> = vec!\[\s*// 255\s*vec!\[255\],\s*\];',
                output,
            ),
            'missing expected concrete counterexample',
        )
        require(
            'kani::concrete_playback_run(concrete_vals, bootstrap_negative);' in output,
            'counterexample is not bound to control',
        )
    else:
        require(
            re.findall(r'^ \*\* (\d+ of \d+ cover properties satisfied)$', output, re.M)
            == ['1 of 1 cover properties satisfied'],
            'missing reachability',
        )
    return [
        {'property': prop, 'status': status, 'description': description}
        for prop, status, description in actual
    ]


def parse_successful_proof(output, returncode, harness):
    """Fail closed on a positive RC proof unless its identity and reachability are explicit."""
    require(harness in ARITHMETIC_HARNESSES, 'unknown reserve proof harness')
    require(returncode == 0, 'reserve proof verifier exited nonzero')
    _reject_infrastructure_diagnostics(output)
    _require_common_kani_identity(output, harness)
    matches = _parse_properties(output)
    require(
        all(match[3] in {'SUCCESS', 'SATISFIED'} for match in matches),
        'reserve proof contains a non-success property status',
    )
    obligation = harness[:4].upper()
    custom_assertions = [
        match
        for match in matches
        if match[3] == 'SUCCESS'
        and match[2].startswith(harness + '.assertion.')
        and obligation in match[4]
    ]
    covers = [
        match
        for match in matches
        if match[3] == 'SATISFIED'
        and match[2].startswith(harness + '.cover.')
        and obligation in match[4]
    ]
    require(custom_assertions, 'named reserve proof assertion is absent')
    require(
        len(covers) == ARITHMETIC_COVER_COUNTS[harness],
        'required reachability inventory is absent or duplicated',
    )
    require(
        all(
            match[5].endswith('in function ' + harness)
            for match in custom_assertions + covers
        ),
        'named reserve proof property is not owned by selected harness',
    )
    require(
        re.findall(r'^VERIFICATION:- (.+)$', output, re.M) == ['SUCCESSFUL'],
        'wrong proof verdict',
    )
    require(
        output.rstrip().endswith(
            'Complete - 1 successfully verified harnesses, 0 failures, 1 total.'
        ),
        'incomplete harness summary',
    )
    return [
        {
            'property': match[2],
            'status': match[3],
            'description': match[4].strip('"'),
        }
        for match in matches
    ]


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def run(command, cwd, timeout, env=None):
    """Kill the whole verifier process group on timeout; preserve partial output."""
    with subprocess.Popen(
        command,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        start_new_session=True,
        env=env,
    ) as process:
        try:
            output, _ = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            output, _ = process.communicate()
            return {
                'command': command,
                'returncode': process.returncode,
                'output': output,
                'timed_out': True,
            }
    return {
        'command': command,
        'returncode': process.returncode,
        'output': output,
        'timed_out': False,
    }


def checked(command, cwd, timeout=30):
    result = run(command, cwd, timeout)
    require(
        not result['timed_out'] and result['returncode'] == 0,
        'command failed: ' + repr(command) + '\n' + result['output'],
    )
    return result['output'].strip()


def proof_environment(kani_home, toolchain):
    """Match the pinned release proxy's subprocess search paths (Kani src/lib.rs)."""
    env = os.environ.copy()
    for name, paths in (
        ('PATH', [kani_home / 'bin', kani_home / 'pyroot/bin']),
        ('PYTHONPATH', [kani_home / 'pyroot']),
    ):
        env[name] = os.pathsep.join(
            [*(str(path) for path in paths), *([env[name]] if env.get(name) else [])]
        )
    env['RUSTUP_TOOLCHAIN'] = toolchain
    if 'LD_LIBRARY_PATH' in env:
        env['LD_LIBRARY_PATH'] = os.pathsep.join(
            path
            for path in env['LD_LIBRARY_PATH'].split(os.pathsep)
            if not (
                len(Path(path).parts) >= 3
                and Path(path).parts[-1] == 'lib'
                and Path(path).parts[-3] == 'toolchains'
            )
        )
    return env


def _source_harnesses(source):
    return re.findall(r'^fn ([a-z0-9_]+)\(', source.read_text(), re.M)


def preflight(kani_home, archive, rustc, mode):
    lock = json.loads((PROOF / 'toolchain-lock.json').read_text())
    require(platform.system() == 'Linux' and platform.machine() == 'x86_64', 'unsupported host')
    require(archive.stat().st_size == lock['release_asset']['size_bytes'], 'archive size mismatch')
    require(digest(archive) == lock['release_asset']['sha256'], 'archive digest mismatch')
    for relative, identity in lock['binaries'].items():
        binary = kani_home / relative
        require(digest(binary) == identity['sha256'], 'binary digest mismatch: ' + relative)
        if 'version' in identity:
            require(
                checked([str(binary), '--version'], ROOT) == identity['version'],
                'binary version mismatch: ' + relative,
            )
    require(checked([str(rustc), '--version'], ROOT) == lock['rustc_version'], 'wrong proof rustc')
    require(
        (kani_home / 'toolchain/bin/rustc').resolve() == rustc.resolve(),
        'Kani compiler toolchain differs from checked rustc',
    )

    bootstrap = PROOF / 'bootstrap.rs'
    bootstrap_source = bootstrap.read_text()
    require(
        _source_harnesses(bootstrap) == list(BOOTSTRAP_HARNESSES)
        and bootstrap_source.count('#[kani::proof]') == len(BOOTSTRAP_HARNESSES)
        and bootstrap_source.count('#[kani::solver(cadical)]') == len(BOOTSTRAP_HARNESSES),
        'unexpected bootstrap harness inventory',
    )

    if mode == 'arithmetic':
        proofs = PROOF / 'proofs.rs'
        proof_source = proofs.read_text()
        require(
            _source_harnesses(proofs) == list(ARITHMETIC_HARNESSES)
            and proof_source.count('#[kani::proof]') == len(ARITHMETIC_HARNESSES)
            and proof_source.count('#[kani::solver(cadical)]') == len(ARITHMETIC_HARNESSES),
            'unexpected arithmetic proof harness inventory',
        )
    return lock


def _run_harness(kani_home, source, harness, lock, timeout=300, concrete=False):
    command = [str(kani_home / 'bin/kani-driver'), str(source), '--harness', harness]
    if concrete:
        command += ['-Z', 'concrete-playback', '--concrete-playback=print']
    return run(
        command,
        source.parent,
        timeout,
        env=proof_environment(kani_home, lock['rust_toolchain']),
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument('--bootstrap', action='store_true', help='run smoke checks only')
    modes.add_argument(
        '--arithmetic',
        action='store_true',
        help='run the RC01/RC02/RC08/RC09 bounded arithmetic proof slice',
    )
    parser.add_argument('--kani-home', type=Path, required=True)
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--rustc', type=Path, required=True)
    parser.add_argument('--output', required=True, type=Path, help='new evidence directory')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    mode = 'bootstrap' if args.bootstrap else 'arithmetic'
    scope = 'tooling-bootstrap-only' if args.bootstrap else 'reserve-arithmetic-proof-slice'
    evidence = {
        'schema_version': 1,
        'scope': scope,
        'accepted': False,
        'reserve_proofs_executed': [],
        'runs': [],
    }
    try:
        evidence['source_sha'] = checked(['git', 'rev-parse', 'HEAD'], ROOT)
        evidence['source_tree'] = checked(['git', 'rev-parse', 'HEAD^{tree}'], ROOT)
        evidence['worktree_status'] = checked(['git', 'status', '--porcelain'], ROOT)
        require(not evidence['worktree_status'], 'proof evidence requires a clean source checkout')
        kani_home, archive, rustc = (
            path.resolve() for path in (args.kani_home, args.archive, args.rustc)
        )
        evidence['tool_lock'] = preflight(kani_home, archive, rustc, mode)

        if args.bootstrap:
            sources = [PROOF / 'bootstrap.rs', PROOF / 'toolchain-lock.json', Path(__file__).resolve()]
            evidence['source_digests'] = {
                str(path.relative_to(ROOT)): digest(path) for path in sources
            }
            with tempfile.TemporaryDirectory(prefix='oregon-bootstrap-') as temp:
                source = Path(temp) / 'bootstrap.rs'
                source.write_bytes((PROOF / 'bootstrap.rs').read_bytes())
                for name in ('bootstrap_positive', 'bootstrap_negative', 'bootstrap_positive'):
                    result = _run_harness(
                        kani_home,
                        source,
                        name,
                        evidence['tool_lock'],
                        timeout=120,
                        concrete=name == 'bootstrap_negative',
                    )
                    evidence['runs'].append(result)
                    require(not result['timed_out'], 'verifier timed out')
                    result['properties'] = parse_bootstrap(
                        result['output'], result['returncode'], name
                    )
        else:
            sources = [
                PROOF / 'proofs.rs',
                PROOF / 'src/model.rs',
                PROOF / 'toolchain-lock.json',
                Path(__file__).resolve(),
            ]
            evidence['source_digests'] = {
                str(path.relative_to(ROOT)): digest(path) for path in sources
            }
            for name in ARITHMETIC_HARNESSES:
                result = _run_harness(
                    kani_home,
                    PROOF / 'proofs.rs',
                    name,
                    evidence['tool_lock'],
                )
                evidence['runs'].append(result)
                require(not result['timed_out'], 'verifier timed out: ' + name)
                result['properties'] = parse_successful_proof(
                    result['output'], result['returncode'], name
                )
                evidence['reserve_proofs_executed'].append(name)
            require(
                evidence['reserve_proofs_executed'] == list(ARITHMETIC_HARNESSES),
                'incomplete arithmetic proof execution',
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
