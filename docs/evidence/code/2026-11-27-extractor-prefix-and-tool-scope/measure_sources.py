#!/usr/bin/env python3
"""Check that explicit preferences and reusable procedures retain their types over ACP."""

import argparse
import asyncio
import json
import os
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / '2026-11-27-memory-relation-references'))
import compare_relations as fixture

CASES = {
    'preference': ('请长期记住我在 Cedar 项目里的个人偏好。今后所有示例日志的开头用 '
                   'MOSS-PRINT-13 作标记。这是我长期偏好的示例格式。',
                   'MOSS-PRINT-13', 'semantic', 'preference'),
    'workflow': ('Remember the reusable Cedar sensor calibration workflow for future releases. '
                 'Start with a zero check, then run FIELD-CHECK-71. If zero drifts by more than '
                 'two units, halt and re-zero before collecting readings.',
                 'FIELD-CHECK-71', 'procedural', 'workflow'),
}


async def run(args):
    args.out.mkdir()
    relay = fixture.base.Relay(os.environ.pop('DEEPSEEK_API_KEY'), args.out / 'provider-usage.jsonl')
    trials = []
    try:
        for index, (case, (prompt, marker, memory_type, source)) in enumerate(CASES.items()):
            order = ('reference', 'candidate') if index == 0 else ('candidate', 'reference')
            for arm in order:
                label = f'{case}-{arm}'
                root = args.out / label
                (root / 'workspace').mkdir(parents=True)
                binary = getattr(args, arm)
                relay.take(label, 1)
                with fixture.endpoint(relay) as (endpoint, capture):
                    async with fixture.connect(binary, root, endpoint, args.embedding) as (client, session):
                        await fixture.prompt(client, session, prompt)
                        try:
                            await fixture.await_records(client, 1)
                        except TimeoutError:
                            pass
                    async with fixture.connect(binary, root, endpoint, args.embedding) as (client, _):
                        result = await fixture.call(client, '_aster/memory/manage', {
                            'workspace_root': str(client.workspace),
                            'request': {'operation': 'list', 'query': '', 'cursor': None, 'memory_type': None},
                        })
                    assert await relay.settle_usage(label)
                returned = result['outcome']['Ok']['items']
                rows = [{'content': row['content'], 'memory_type': row['memory_type'],
                         'source': row['metadata'].get('source'),
                         'evolution_kind': row['metadata'].get('evolution_kind')} for row in returned]
                receipts = [receipt for receipt in relay.rows if receipt['trial'] == label]
                usage = fixture.base.summarise_usage(receipts)
                assert len(capture) == 1 and not capture[0]['related_rows'] and usage['usage_complete']
                passed = any(marker in row['content'] and row['memory_type'] == memory_type
                             and row['source'] == source for row in rows)
                trials.append({'case': case, 'arm': arm, 'passed': passed, 'records': rows,
                               'proposal': capture[0]['proposal'], **usage})
                (args.out / 'trials.json').write_text(json.dumps(trials, ensure_ascii=False, indent=2) + '\n')
                print(label, passed, usage['tokens']['total_tokens'], flush=True)
    finally:
        relay.server.shutdown()
        relay.server.server_close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('reference', 'candidate', 'embedding', 'out'):
        parser.add_argument('--' + name, type=lambda value: Path(value).resolve(), required=True)
    asyncio.run(run(parser.parse_args()))
