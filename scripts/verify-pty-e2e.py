#!/usr/bin/env python3
"""Run the deterministic ACP PTY end-to-end suites in one command.

This is an opt-in verification tool. It does not change the default build,
any CI workflow, or the shipped artifacts. Every suite is executed with the
pinned toolchain through `cargo test --locked`, one at a time.

    python3 scripts/verify-pty-e2e.py
    python3 scripts/verify-pty-e2e.py --suite acp_conformance_stdio
    python3 scripts/verify-pty-e2e.py --exclude tui_913_hermetic --exclude startup_error

Exit code 0 means every selected suite passed; 1 means at least one failed.
Suites with known open failures stay in the default set so they stay visible.
"""
from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import subprocess
import sys
import tempfile
import time

REPO = pathlib.Path(__file__).resolve().parent.parent

SUITES = [
    ('shell_bypass_learning', False),
    ('memory_management', False),
    ('memory_canonical_stdio', False),
    ('acp_conformance_stdio', False),
    ('protocol_commands_stdio', False),
    ('startup_error', True),
    ('workspace_layer_trust_stdio', False),
    ('unattended_e2e', False),
    ('shipped_tui_e2e', False),
    ('tui_913_hermetic', True),
]

RESULT_RE = re.compile(
    r'test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored')
FAILED_RE = re.compile(r'^test (\S+) \.\.\. FAILED', re.M)


def run_suite(name: str, log_dir: pathlib.Path) -> dict:
    log = log_dir / f'{name}.log'
    env = dict(os.environ,
               CARGO_TARGET_DIR=str(REPO / 'target'), CARGO_INCREMENTAL='0')
    env.pop('RUSTC_WRAPPER', None)
    command = ['cargo', 'test', '--locked', '-p', 'aster-acp', '--test', name,
               '--', '--test-threads=1']
    started = time.perf_counter()
    try:
        proc = subprocess.run(command, cwd=REPO, env=env, capture_output=True,
                              text=True, timeout=900)
    except subprocess.TimeoutExpired as exc:
        return {'suite': name, 'exit': 124, 'timed_out': True,
                'passed': 0, 'failed': 0, 'ignored': 0,
                'failed_tests': [], 'log': str(log)}
    elapsed = round(time.perf_counter() - started, 2)
    log.write_text((proc.stdout or '') + '\n' + (proc.stderr or ''))
    matches = RESULT_RE.findall(proc.stdout or '')
    passed, failed, ignored = map(int, matches[-1]) if matches else (0, 0, 0)
    return {'suite': name, 'exit': proc.returncode, 'seconds': elapsed,
            'passed': passed, 'failed': failed, 'ignored': ignored,
            'log': str(log),
            'failed_tests': FAILED_RE.findall(proc.stdout or '')}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--suite', action='append', default=[],
                        help='run only these suites (repeatable)')
    parser.add_argument('--exclude', action='append', default=[],
                        help='skip these suites (repeatable)')
    parser.add_argument('--out', type=pathlib.Path,
                        help='write a JSON summary to this path')
    parser.add_argument('--log-dir', type=pathlib.Path,
                        default=pathlib.Path(tempfile.gettempdir()) / 'aster-pty-verify-logs',
                        help='directory for per-suite logs')
    args = parser.parse_args()

    selected = [(name, known) for name, known in SUITES
                if (not args.suite or name in args.suite)
                and name not in args.exclude]
    if not selected:
        parser.error('no suites selected')

    args.log_dir.mkdir(parents=True, exist_ok=True)
    results = []
    for name, known in selected:
        row = run_suite(name, args.log_dir)
        row['known_open_failures'] = known
        results.append(row)
        print(json.dumps(row, ensure_ascii=False), flush=True)

    summary = {
        'suites': len(results),
        'passed': sum(r['passed'] for r in results),
        'failed': sum(r['failed'] for r in results),
        'ignored': sum(r['ignored'] for r in results),
        'seconds': round(sum(r.get('seconds', 0) for r in results), 2),
        'failed_suites': [r['suite'] for r in results
                          if r['failed'] or r['exit'] or r.get('timed_out')],
        'default_build_unchanged': True,
        'results': results,
    }
    if args.out:
        args.out.write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps({key: summary[key] for key in
                      ('suites', 'passed', 'failed', 'ignored', 'seconds',
                       'failed_suites')}, indent=2), flush=True)
    sys.exit(1 if summary['failed_suites'] else 0)


if __name__ == '__main__':
    main()
