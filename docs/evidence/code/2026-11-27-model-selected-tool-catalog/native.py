#!/usr/bin/env python3
"""Use the real Core catalog mode with the unchanged frozen ACP grader."""

import argparse
import asyncio
from pathlib import Path
import sys

from pilot import base


def candidate_with_catalog(directory, acp):
    directory.mkdir(exist_ok=True)
    launcher = directory / 'deferred-acp'
    launcher.write_text(
        f'#!{sys.executable}\n'
        'import os, sys\n'
        'with open(os.environ["ASTER_CONFIG"], "a", encoding="utf-8") as config:\n'
        '    config.write(\'\\ntool_presentation { mode = "deferred" }\\n\')\n'
        f'acp = {str(acp)!r}\n'
        'os.execv(acp, [acp, *sys.argv[1:]])\n',
        encoding='utf-8',
    )
    launcher.chmod(0o700)
    return launcher


def main():
    parser = argparse.ArgumentParser()
    for name in ('release', 'acp', 'dsh', 'embedding', 'out'):
        parser.add_argument('--' + name, type=lambda value: Path(value).resolve(), required=name != 'embedding')
    parser.add_argument('--reference-deferred', action='store_true')
    parser.add_argument('--order', nargs='+', required=True)
    args = parser.parse_args()
    args.candidate = candidate_with_catalog(args.out.with_name(args.out.name + '-launcher'), args.acp)
    if args.reference_deferred:
        args.release = candidate_with_catalog(args.out.with_name(args.out.name + '-reference'), args.release)
    asyncio.run(base.evaluate(args))


if __name__ == '__main__':
    main()
