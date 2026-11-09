#!/usr/bin/env python3
"""Run Rusk's checks, local tests, sanitizers, and public judge verification."""
import argparse
from dataclasses import dataclass
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


@dataclass
class Result:
    name: str
    passed: bool
    seconds: float


def run_stage(name, commands, results, *, env=None):
    print(f'\n=== {name} ===', flush=True)
    start = time.monotonic()
    passed = True
    for command in commands:
        print('$ ' + ' '.join(map(str, command)), flush=True)
        try:
            completed = subprocess.run(command, cwd=ROOT, env=env, check=False)
        except OSError as error:
            print(f'{name}: {error}', file=sys.stderr, flush=True)
            passed = False
            break
        if completed.returncode != 0:
            passed = False
            # A failed prerequisite must not leave us testing an old executable.
            break
    results.append(Result(name, passed, time.monotonic() - start))


def run_public(results):
    sources = sorted((ROOT / 'examples').rglob('*.test.rusk'))
    virtual_bin = ROOT / '.venv-verify' / 'bin'
    env = os.environ.copy()
    env['PATH'] = str(virtual_bin) + os.pathsep + env.get('PATH', '')
    verifier = shutil.which('oj-verify', path=env['PATH'])
    if not verifier or not shutil.which('oj', path=env['PATH']):
        print('\nPublic judges: install requirements-verify.txt first:', file=sys.stderr)
        print('  python3 -m venv .venv-verify', file=sys.stderr)
        print('  .venv-verify/bin/python -m pip install -r requirements-verify.txt', file=sys.stderr)
        results.append(Result('Public judges', False, 0))
        return
    if not sources:
        print('Public judges: no *.test.rusk files found in examples/', file=sys.stderr)
        results.append(Result('Public judges', False, 0))
        return
    run_stage(
        'Public judges',
        [[verifier, 'run', '--timeout', '3000', *[str(p.relative_to(ROOT)) for p in sources]]],
        results,
        env=env,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        '--suite', choices=('all', 'local', 'sanitizers', 'public'), default='all',
        help='all (default), local (fmt/clippy/Cargo), sanitizers, or public judges',
    )
    parser.add_argument('--no-leaks', action='store_true', help='disable LeakSanitizer in ptrace environments')
    args = parser.parse_args()
    results = []
    try:
        if args.suite in ('all', 'local'):
            run_stage('Formatting', [['cargo', 'fmt', '--check']], results)
            run_stage('Clippy', [['cargo', 'clippy', '--locked', '--all-targets', '--', '-D', 'warnings']], results)
            run_stage('Cargo tests (E2E + AtCoder)', [['cargo', 'test', '--locked']], results)
            reference_env = os.environ.copy()
            reference_env['RUSK_TEST_NO_OPT'] = '1'
            reference_command = ['cargo', 'test', '--locked']
            for test in ('e2e', 'records', 'tuples', 'generics', 'includes', 'data_structures', 'callables', 'higher_order', 'beta04_structures', 'beta05', 'pipeline', 'blocks_labels', 'enums', 'test_support', 'beta06', 'minmax', 'effects', 'bindings', 'readonly', 'readonly_routes', 'documentation', 'noescape', 'noescape_routes', 'heap_aliases'):
                reference_command += ['--test', test]
            run_stage('Reference backend (no scalar optimization)',
                      [reference_command], results, env=reference_env)
        if args.suite in ('all', 'sanitizers'):
            command = [sys.executable, str(ROOT / 'tools/check_sanitizers.py')]
            if args.no_leaks:
                command.append('--no-leaks')
            run_stage('Sanitizers', [['cargo', 'build', '--locked'], command], results)
        if args.suite in ('all', 'public'):
            run_public(results)
    except KeyboardInterrupt:
        print('\nInterrupted.', file=sys.stderr)
        return 130
    print('\n=== Summary ===', flush=True)
    for result in results:
        status = 'PASS' if result.passed else 'FAIL'
        print(f'{status:4}  {result.name} ({result.seconds:.1f}s)', flush=True)
    passed = all(result.passed for result in results)
    print('All selected checks passed.' if passed else 'Some checks failed.', flush=True)
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
