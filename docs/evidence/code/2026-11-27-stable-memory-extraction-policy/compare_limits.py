#!/usr/bin/env python3
"""Replay the frozen memory tasks through different extraction item limits."""

import argparse
import asyncio
import json
import os
from pathlib import Path
import sys
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / '2026-11-27-extractor-prefix-and-tool-scope'))
from measure_cache import configure, fixture


async def compare(args):
    args.out.mkdir()
    credential = os.environ.pop('DEEPSEEK_API_KEY')
    manifest = {
        'model': fixture.base.MODEL,
        'order': args.order,
        'reference_limit': args.low,
        'candidate_limit': args.high,
        'frozen_cases': list(fixture.CASES),
    }
    (args.out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    for arm in args.order:
        binary = getattr(args, arm)
        options = {}
        for label, limit in (('reference', args.low), ('candidate', args.high)):
            directory = args.out / f'{arm}-limit-{limit}'
            directory.mkdir()
            options[label] = configure(directory, binary, limit)
        os.environ['DEEPSEEK_API_KEY'] = credential
        await fixture.evaluate(SimpleNamespace(
            **options, embedding=args.embedding, seed_directory=args.seed_directory,
            out=args.out / arm,
        ))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ('reference', 'candidate', 'embedding', 'out', 'seed-directory'):
        parser.add_argument('--' + option, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--low', type=int, choices=range(1, 10), required=True)
    parser.add_argument('--high', type=int, choices=range(2, 11), required=True)
    parser.add_argument('--order', choices=['reference', 'candidate'], nargs=2, required=True)
    args = parser.parse_args()
    if args.low >= args.high or len(set(args.order)) != 2:
        parser.error('provide ascending limits and run both builds')
    asyncio.run(compare(args))
