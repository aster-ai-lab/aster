#!/usr/bin/env python3
"""Make opt-in lightweight copies of the default aster / aster-acp pair.

This is NOT part of the default or release build. The default artifacts under
target/debug are only read, never modified. Enable by running this script;
disable by not running it. To go back to the normal pair, simply use the
default artifacts (or run `just build`); delete the output directory when the
copies are no longer needed.

`strip -x` removes local symbols but keeps every exported symbol and every
loaded dylib, so the copies remain runnable and keep their BUILD_ID. They are
NOT suitable for debugging (local symbols are gone). Distribution archives
must continue to use the default build.
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_TARGET = (REPO / 'target' / 'debug').resolve()
PAIR = ('aster', 'aster-acp')


def run(argv: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(argv, capture_output=True, text=True)


def line_count(argv: list[str], path: Path) -> int:
    result = run([*argv, str(path)])
    if result.returncode != 0:
        raise RuntimeError(f'{argv[0]} failed on {path}: {result.stderr.strip()[:200]}')
    return len(result.stdout.splitlines())


def build_id(path: Path) -> str:
    result = run([str(path), '--build-id'])
    if result.returncode != 0:
        raise RuntimeError(f'{path.name} --build-id failed: {result.stderr.strip()[:200]}')
    return result.stdout.strip()


def preflight() -> None:
    if sys.platform != 'darwin':
        raise SystemExit('aster-small-artifacts supports macOS only (strip -x / dyld_info)')
    for tool in ('strip', 'dyld_info', 'otool'):
        if shutil.which(tool) is None:
            raise SystemExit(f'required tool not found: {tool}')
    for name in PAIR:
        source = DEFAULT_TARGET / name
        if not source.is_file():
            raise SystemExit(f'default artifact missing: {source}. Run `just build` first.')


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', required=True, type=Path,
                        help='output directory for the lightweight copies (never target/debug)')
    parser.add_argument('--force', action='store_true',
                        help='overwrite existing copies in --out')
    args = parser.parse_args()

    preflight()
    out = args.out.expanduser().resolve()
    if out == DEFAULT_TARGET or DEFAULT_TARGET in out.parents:
        raise SystemExit('--out must not be the default target/debug directory')
    out.mkdir(parents=True, exist_ok=True)
    if not out.is_dir():
        raise SystemExit(f'--out is not a directory: {out}')

    report = {'mode': 'opt-in-small-artifacts', 'default_target': str(DEFAULT_TARGET),
              'out': str(out), 'artifacts': []}
    for name in PAIR:
        source = DEFAULT_TARGET / name
        destination = out / name
        if destination.exists() and not args.force:
            raise SystemExit(f'{destination} already exists; pass --force to overwrite')

        before_bytes = source.stat().st_size
        exports_before = line_count(['dyld_info', '-exports'], source)
        imports_before = line_count(['otool', '-L'], source)
        build_before = build_id(source)

        shutil.copy2(source, destination)
        subprocess.run(['strip', '-x', str(destination)], check=True,
                       capture_output=True, text=True)

        after_bytes = destination.stat().st_size
        exports_after = line_count(['dyld_info', '-exports'], destination)
        imports_after = line_count(['otool', '-L'], destination)
        build_after = build_id(destination)

        if exports_after != exports_before or imports_after != imports_before:
            raise SystemExit(f'{name}: export/import count changed '
                            f'({exports_before}->{exports_after}, '
                            f'{imports_before}->{imports_after})')
        if build_after != build_before:
            raise SystemExit(f'{name}: BUILD_ID changed after strip -x')
        if after_bytes >= before_bytes:
            raise SystemExit(f'{name}: strip -x did not reduce size')

        report['artifacts'].append({
            'name': name,
            'bytes_before': before_bytes,
            'bytes_after': after_bytes,
            'bytes_saved': before_bytes - after_bytes,
            'pct': round(100.0 * (before_bytes - after_bytes) / before_bytes, 2),
            'dyld_exports': exports_before,
            'otool_imports': imports_before,
            'build_id': build_after,
            'runnable': True,
        })

    report['bytes_saved_total'] = sum(a['bytes_saved'] for a in report['artifacts'])
    (out / 'small-artifacts.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
