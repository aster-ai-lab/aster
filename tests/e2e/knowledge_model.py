#!/usr/bin/env python3
"""One explicit loopback knowledge case; the real ACP reads and writes staging."""
import argparse
import json
from http.server import ThreadingHTTPServer
from pathlib import Path

from json_render_model import Handler

CASE = 'knowledge-basic-ingest-v1'
SOURCE_PATH = '实验室计划.md'
SOURCE = '# 晨星实验室\n\n负责人：林夏。\n试验区：北区。\n每周二检查设备，维护周期为 14 天。\n'
PAGE_PATH = 'concepts/晨星实验室维护计划.md'
PAGE = ('---\ntype: concept\ntitle: 晨星实验室维护计划\n---\n\n'
        '# 晨星实验室维护计划\n\n负责人是林夏，试验区位于北区。'
        '每周二检查设备，维护周期为 14 天。\n\n'
        '[资料](../实验室计划.md)\n')
MANIFEST = {'sources': [SOURCE_PATH], 'summary': '依据实验室计划整理维护周期及负责人。', 'pages': [
    {'path': PAGE_PATH, 'title': '晨星实验室维护计划', 'kind': 'concept', 'change': 'create',
     'reason': '原始资料明确记录负责人、试验区和维护安排。', 'requires_review': False,
     'sources': [SOURCE_PATH]}]}
STEPS = [
    ('kn-basic-task', 'read', {'file_path': '.wiki-task.md'}),
    ('kn-basic-source', 'read', {'file_path': SOURCE_PATH}),
    ('kn-basic-page', 'write', {'file_path': PAGE_PATH, 'content': PAGE}),
    ('kn-basic-manifest', 'write', {'file_path': '.wiki-changes.json',
                                 'content': json.dumps(MANIFEST, ensure_ascii=False)}),
]


def decide(body):
    if body.get('model') != CASE:
        raise ValueError('unknown knowledge fixture model')
    tools = {tool.get('function', {}).get('name') for tool in body.get('tools', [])}
    if not tools:
        return {'role': 'assistant', 'content': '{"items":[]}'}, 'stop', {'case': CASE, 'kind': 'no_tools'}
    if not {'read', 'write'} <= tools:
        raise ValueError('knowledge fixture requires the real read and write tools')
    messages = body.get('messages', [])
    results = [message for message in messages if message.get('role') == 'tool']
    calls = [call for message in messages if message.get('role') == 'assistant'
             for call in message.get('tool_calls', [])]
    if len(results) > len(STEPS) or len(calls) != len(results):
        raise ValueError('unexpected extra or missing tool results')
    if len({call.get('id') for call in calls}) != len(calls):
        raise ValueError('duplicate knowledge tool identity')
    for result, call, (call_id, tool, args) in zip(results, calls, STEPS):
        function = call.get('function', {})
        if (not call.get('id') or result.get('tool_call_id') != call['id']
                or not isinstance(result.get('content'), str)
                or function.get('name') != tool
                or json.loads(function.get('arguments', 'null')) != args):
            raise ValueError('uncorrelated knowledge tool result')
        status, separator, content = result['content'].partition('\n')
        if not separator or status != 'Tool exit code: 0':
            raise ValueError('knowledge tool did not report a successful exit')
        if tool == 'read':
            lines = [line.split('\t', 1)[-1] for line in content.splitlines()]
            if call_id == 'kn-basic-task':
                if not all(fragment in '\n'.join(lines) for fragment in [
                    'Operation: ingest.', SOURCE_PATH, '.wiki-changes.json',
                ]):
                    raise ValueError('work order does not match this fixture')
            elif not all(line in lines for line in SOURCE.splitlines() if line):
                raise ValueError('source read does not match the frozen fixture')
        elif not content.startswith(f'Wrote {len(args["content"].encode())} bytes '):
            raise ValueError('write did not return the expected byte receipt')
    event = {'case': CASE, 'completed_tools': len(results)}
    if len(results) == len(STEPS):
        return {'role': 'assistant', 'content': 'KNOWLEDGE-BASIC-STAGED'}, 'stop', event
    call_id, tool, args = STEPS[len(results)]
    call = {'id': call_id, 'type': 'function', 'function': {
        'name': tool, 'arguments': json.dumps(args, ensure_ascii=False)}}
    event.update(tool_call_id=call_id, tool=tool)
    return {'role': 'assistant', 'content': None, 'tool_calls': [call]}, 'tool_calls', event


class KnowledgeHandler(Handler):
    model_name = CASE
    decide = staticmethod(decide)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=0)
    parser.add_argument('--events', type=Path, required=True)
    args = parser.parse_args()
    args.events.parent.mkdir(parents=True, exist_ok=True)
    server = ThreadingHTTPServer(('127.0.0.1', args.port), KnowledgeHandler)
    server.events = args.events
    print(json.dumps({'baseUrl': f'http://127.0.0.1:{server.server_port}/v1', 'model': CASE}), flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
