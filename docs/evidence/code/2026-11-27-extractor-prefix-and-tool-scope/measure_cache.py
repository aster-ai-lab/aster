#!/usr/bin/env python3
"""Measure DeepSeek input cache reuse when an actual ACP extraction changes recall shape."""

import argparse
import asyncio
import json
import os
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / '2026-11-27-memory-relation-references'))
import compare_relations as fixture


def configure(directory, acp, max_items):
    launcher = directory / 'acp'
    launcher.write_text(
        f'#!{sys.executable}\n'
        'import os, sys\n'
        'with open(os.environ["ASTER_CONFIG"], "a", encoding="utf-8") as config:\n'
        f'    config.write({f"\nmemory {{ llm_extraction_max_items = {max_items} }}\n"!r})\n'
        f'acp = {str(acp)!r}\n'
        'os.execv(acp, [acp, *sys.argv[1:]])\n',
        encoding='utf-8',
    )
    launcher.chmod(0o700)
    return launcher


async def measure(args):
    args.out.mkdir()
    relay = fixture.base.Relay(os.environ.pop('DEEPSEEK_API_KEY'), args.out / 'provider-usage.jsonl')
    trials = []
    # Each transition gets its own supported first-line item limit so a previous
    # trial has not already warmed the exact shape that is about to be tested.
    controls = (
        ('reference', args.first_limit, (0, 4)),
        ('candidate', args.first_limit + 1, (0, 4)),
        ('candidate', args.first_limit + 2, (4, 0)),
        ('reference', args.first_limit + 3, (4, 0)),
    )
    try:
        for arm, max_items, shapes in controls:
            directory = args.out / f'{arm}-{max_items}'
            directory.mkdir()
            launcher = configure(directory, getattr(args, arm), max_items)
            group = []
            for index, count in enumerate(shapes):
                root = directory / f'recall-{count}'
                (root / 'workspace').mkdir(parents=True)
                if count:
                    path = Path('workspace/.aster/memory')
                    fixture.clone_memory(args.seed_directory / path, root / path)
                label = f'{directory.name}-{count}'
                relay.take(label, index + 1)
                with fixture.endpoint(relay) as (endpoint, requests):
                    async with fixture.connect(launcher, root, endpoint, args.embedding) as (client, session):
                        await fixture.prompt(client, session, fixture.CASES['replacement'][0])
                        try:
                            await fixture.await_records(client, count + 1)
                        except TimeoutError:
                            pass
                        observed = await fixture.records(client)
                    assert await relay.settle_usage(label)
                records = [row for row in relay.rows if row['trial'] == label]
                usage = fixture.base.summarise_usage(records)
                assert len(requests) == len(records) == 1 and usage['usage_complete']
                request = requests[0]
                assert len(request['related_rows']) == count
                updated = [row for row in observed if '09:30' in row['content']]
                old = next((row for row in observed if row['content'] == fixture.OLD), None)
                passed = len(updated) == 1 and (count == 0 or (
                    old is not None and updated[0]['metadata'].get('supersedes') == old['id']
                    and old['metadata'].get('superseded_by') == updated[0]['id']
                    and old['metadata'].get('aster.memory.status') == 'superseded'))
                group.append({'label': label, 'related_count': count, 'stored': passed,
                              **usage['tokens'], 'request': request})
            trials.append({'arm': arm, 'max_items': max_items, 'shapes': shapes, 'requests': group})
            (args.out / 'trials.json').write_text(json.dumps(trials, ensure_ascii=False, indent=2) + '\n')
            print(arm, shapes, [(row['prompt_tokens'], row['prompt_cache_hit_tokens'], row['stored'])
                                for row in group], flush=True)
    finally:
        relay.server.shutdown()
        relay.server.server_close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ('reference', 'candidate', 'embedding', 'out', 'seed-directory'):
        parser.add_argument('--' + option, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--first-limit', type=int, choices=range(1, 8), default=6)
    asyncio.run(measure(parser.parse_args()))
