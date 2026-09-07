"""Fail-closed Kani 0.67.0 bootstrap. This does not execute RC01–RC10."""
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


class GateError(ValueError):
    """Evidence does not establish the expected verification result."""


def require(condition, message):
    if not condition:
        raise GateError(message)


def parse_bootstrap(output, returncode, harness):
    """Accept only the observed smoke property inventory and semantic control."""
    require(harness in ('bootstrap_positive', 'bootstrap_negative'), 'unknown harness')
    negative = harness == 'bootstrap_negative'
    require(returncode == int(negative), 'unexpected verifier exit status')
    require(not re.search(r'(?im)^.*(?:error:|error\[|unsupported|unwinding assertion|timed out)', output),
            'infrastructure or unsupported-operation diagnostic')
    for marker in ('Kani Rust Verifier 0.67.0 (standalone)',
                   'Checking harness ' + harness + '...', 'RESULTS:', 'SUMMARY:',
                   'Manual Harness Summary:'):
        require(output.count(marker) == 1, 'missing or repeated output marker: ' + marker)
    require(re.findall(r'^Checking harness (.+)\.\.\.$', output, re.M) == [harness],
            'unexpected harness inventory')
    require(re.findall(r'^CBMC[^\n]*$', output, re.M) == [
        'CBMC 6.8.0 (cbmc-6.8.0)',
        'CBMC version 6.8.0 (cbmc-6.8.0) 64-bit x86_64 linux',
    ], 'wrong or repeated backend inventory')
    solvers = re.findall(r'^Solving with (.+)$', output, re.M)
    require(solvers and set(solvers) == {'CaDiCaL 2.0.0'}, 'wrong or absent solver')
    section = output.split('RESULTS:\n', 1)[-1].split('SUMMARY:', 1)[0]
    pattern = re.compile(
        r'Check (\d+): ([^\n]+)\n\s+- Status: ([A-Z]+)\n'
        r'\s+- Description: ([^\n]+)\n\s+- Location: ([^\n]+)\n')
    matches = list(pattern.finditer(section))
    require(not pattern.sub('', section).strip(), 'unparsed property output')
    require(re.findall(r'^Check [^\n]*$', output, re.M)
            == re.findall(r'^Check [^\n]*$', section, re.M),
            'property output outside results section')
    if negative:
        expected = [('bootstrap_negative.assertion.1', 'FAILURE',
                     'bootstrap wrapping increment must produce a counterexample')]
    else:
        expected = [('bootstrap_positive.assertion.1', 'SUCCESS', 'attempt to add with overflow'),
                    ('bootstrap_positive.assertion.2', 'SUCCESS', 'bootstrap widening preserves increment'),
                    ('bootstrap_positive.cover.1', 'SATISFIED', 'maximum input is reachable')]
    actual = [(m[2], m[3], m[4].strip('"')) for m in matches]
    require(actual == expected, 'unexpected properties, statuses or descriptions')
    require([int(m[1]) for m in matches] == list(range(1, len(expected) + 1)),
            'nonsequential property inventory')
    require(all(m[5].endswith('in function ' + harness) for m in matches), 'wrong property location')
    summary = '1 of 1 failed' if negative else '0 of 2 failed'
    require(re.findall(r'^ \*\* (\d+ of \d+ failed)$', output, re.M) == [summary],
            'inconsistent assertion summary')
    verdict = 'FAILED' if negative else 'SUCCESSFUL'
    require(re.findall(r'^VERIFICATION:- (.+)$', output, re.M) == [verdict], 'wrong verdict')
    completed = ('Complete - 0 successfully verified harnesses, 1 failures, 1 total.' if negative
                 else 'Complete - 1 successfully verified harnesses, 0 failures, 1 total.')
    require(re.findall(r'^Complete - [^\n]*$', output, re.M) == [completed],
            'unexpected completion inventory')
    require(output.rstrip().endswith(completed), 'incomplete harness summary')
    if negative:
        require(re.search(r'let concrete_vals: Vec<Vec<u8>> = vec!\[\s*// 255\s*vec!\[255\],\s*\];', output),
                'missing expected concrete counterexample')
        require('kani::concrete_playback_run(concrete_vals, bootstrap_negative);' in output,
                'counterexample is not bound to control')
    else:
        require(re.findall(r'^ \*\* (\d+ of \d+ cover properties satisfied)$', output, re.M)
                == ['1 of 1 cover properties satisfied'], 'missing reachability')
    return [{'property': p, 'status': s, 'description': d} for p, s, d in actual]


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def run(command, cwd, timeout, env=None):
    """Kill the whole verifier process group on timeout; preserve partial output."""
    with subprocess.Popen(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                          text=True, start_new_session=True, env=env) as process:
        try:
            output, _ = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            output, _ = process.communicate()
            return {'command': command, 'returncode': process.returncode,
                    'output': output, 'timed_out': True}
    return {'command': command, 'returncode': process.returncode,
            'output': output, 'timed_out': False}


def checked(command, cwd, timeout=30):
    result = run(command, cwd, timeout)
    require(not result['timed_out'] and result['returncode'] == 0,
            'command failed: ' + repr(command) + '\n' + result['output'])
    return result['output'].strip()



def proof_environment(kani_home, toolchain):
    """Match the pinned release proxy's subprocess search paths (Kani src/lib.rs)."""
    env = os.environ.copy()
    for name, paths in (
        ('PATH', [kani_home / 'bin', kani_home / 'pyroot/bin']),
        ('PYTHONPATH', [kani_home / 'pyroot']),
    ):
        env[name] = os.pathsep.join([*(str(p) for p in paths), *([env[name]] if env.get(name) else [])])
    env['RUSTUP_TOOLCHAIN'] = toolchain
    if 'LD_LIBRARY_PATH' in env:
        env['LD_LIBRARY_PATH'] = os.pathsep.join(
            p for p in env['LD_LIBRARY_PATH'].split(os.pathsep)
            if not (len(Path(p).parts) >= 3 and Path(p).parts[-1] == 'lib'
                    and Path(p).parts[-3] == 'toolchains'))
    return env


def preflight(kani_home, archive, rustc):
    lock = json.loads((PROOF / 'toolchain-lock.json').read_text())
    require(platform.system() == 'Linux' and platform.machine() == 'x86_64', 'unsupported host')
    require(archive.stat().st_size == lock['release_asset']['size_bytes'], 'archive size mismatch')
    require(digest(archive) == lock['release_asset']['sha256'], 'archive digest mismatch')
    for relative, identity in lock['binaries'].items():
        binary = kani_home / relative
        require(digest(binary) == identity['sha256'], 'binary digest mismatch: ' + relative)
        if 'version' in identity:
            require(checked([str(binary), '--version'], ROOT) == identity['version'],
                    'binary version mismatch: ' + relative)
    require(checked([str(rustc), '--version'], ROOT) == lock['rustc_version'], 'wrong proof rustc')
    require((kani_home / 'toolchain/bin/rustc').resolve() == rustc.resolve(),
            'Kani compiler toolchain differs from checked rustc')
    source = (PROOF / 'bootstrap.rs').read_text()
    require(re.findall(r'^fn (\w+)\(', source, re.M) == ['bootstrap_positive', 'bootstrap_negative']
            and source.count('#[kani::proof]') == 2
            and source.count('#[kani::solver(cadical)]') == 2, 'unexpected source harness inventory')
    return lock


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bootstrap', action='store_true', help='run smoke checks only')
    parser.add_argument('--kani-home', type=Path)
    parser.add_argument('--archive', type=Path)
    parser.add_argument('--rustc', type=Path)
    parser.add_argument('--output', required=True, type=Path, help='new evidence directory')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    evidence = {'schema_version': 1, 'scope': 'tooling-bootstrap-only', 'accepted': False,
                'reserve_proofs_executed': [], 'runs': []}
    try:
        evidence['source_sha'] = checked(['git', 'rev-parse', 'HEAD'], ROOT)
        evidence['source_tree'] = checked(['git', 'rev-parse', 'HEAD^{tree}'], ROOT)
        evidence['worktree_status'] = checked(['git', 'status', '--porcelain'], ROOT)
        require(args.bootstrap, 'RC01–RC10 runner is not implemented; select --bootstrap for tooling only')
        require(all((args.kani_home, args.archive, args.rustc)),
                '--kani-home, --archive and --rustc are required for pinned bootstrap')
        require(not evidence['worktree_status'], 'proof evidence requires a clean source checkout')
        kani_home, archive, rustc = (p.resolve() for p in (args.kani_home, args.archive, args.rustc))
        evidence['tool_lock'] = preflight(kani_home, archive, rustc)
        evidence['source_digests'] = {
            str(p.relative_to(ROOT)): digest(p) for p in
            (PROOF / 'bootstrap.rs', PROOF / 'toolchain-lock.json', Path(__file__).resolve())}
        with tempfile.TemporaryDirectory(prefix='oregon-bootstrap-') as temp:
            source = Path(temp) / 'bootstrap.rs'
            source.write_bytes((PROOF / 'bootstrap.rs').read_bytes())
            for name in ('bootstrap_positive', 'bootstrap_negative', 'bootstrap_positive'):
                command = [str(kani_home / 'bin/kani-driver'), str(source), '--harness', name]
                if name == 'bootstrap_negative':
                    command += ['-Z', 'concrete-playback', '--concrete-playback=print']
                result = run(command, temp, 120,
                             env=proof_environment(kani_home, evidence['tool_lock']['rust_toolchain']))
                evidence['runs'].append(result)
                require(not result['timed_out'], 'verifier timed out')
                result['properties'] = parse_bootstrap(result['output'], result['returncode'], name)
        evidence['accepted'] = True
    except (GateError, OSError, ValueError) as error:
        evidence['error'] = str(error)
    (args.output / 'summary.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps({'accepted': evidence['accepted'], 'scope': evidence['scope'],
                      'error': evidence.get('error')}))
    return 0 if evidence['accepted'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
