#!/usr/bin/env python3
"""Use actual ACP reads and a real DeepSeek answer to measure line-number projection."""

import argparse
import asyncio
import hashlib
import json
import os
import re
import sys
from contextlib import contextmanager
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from threading import Thread

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / '2026-11-27-memory-relation-references'))
import compare_relations as acp_fixture


def cases(shape):
    path = '青石 ledger.rs'
    rows = [f'    let stage_{i} = read_slot({i}, {i % 17});' for i in range(1, 1551)]
    rows[1196] = '    let obsolete_threshold = read_slot(147, 19);'
    code = '\n'.join(rows) + '\n'
    config_path = '划分 state.yaml'
    rows = [f'    value_{i}: "节点-{i}"' for i in range(1, 611)]
    rows[438] = '     value_seed: "青石-73"'
    config = '\n'.join(rows) + '\n'
    trials = {
        'range': (
            {path: code}, {'file_path': path, 'offset': 800, 'limit': 620},
            f'Find the obsolete_threshold assignment in {path}. Return only JSON with keys line, slot, fallback and the exact displayed line number and two numeric arguments. Use the tool evidence already provided.',
            {'line': 1197, 'slot': 147, 'fallback': 19},
        ),
        'files': (
            {path: code, config_path: config},
            {'files': [{'path': config_path}, {'path': path, 'offset': 900, 'limit': 510}]},
            f'Find value_seed in {config_path} and obsolete_threshold in {path}. Return only JSON with keys config_line, code_line, seed. Use the displayed line numbers and exact seed value from the tools already provided.',
            {'config_line': 439, 'code_line': 1197, 'seed': '青石-73'},
        ),
    }
    if shape == 'large':
        return trials
    files, _, prompt, expected = trials['range']
    trials['range'] = (files, {'file_path': path, 'offset': 1000, 'limit': 320}, prompt, expected)
    files, partial_args, prompt, expected = trials['files']
    trials['files'] = (
        files,
        {'files': [{'path': config_path, 'offset': 300, 'limit': 150},
                   {'path': path, 'offset': 1120, 'limit': 150}]},
        prompt, expected,
    )
    trials['files-page'] = (
        files, partial_args,
        f'Find value_seed in {config_path} in the displayed page. Return only JSON with keys config_line, seed, next_offset. Use its exact displayed line and seed, and the next offset for this file from the existing read.files continuation cursor.',
        {'config_line': 439, 'seed': '青石-73', 'next_offset': 468},
    )
    return trials


def digest(body):
    return hashlib.sha256(body.encode()).hexdigest()


def unpadded_digest(body):
    return digest(re.sub(r'^ +(?=[0-9]+\t)', '', body, flags=re.M))


@contextmanager
def endpoint(relay, args):
    awaiting_read = True
    captured = []

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def reply(self, code, kind, body):
            self.send_response(code)
            self.send_header('Content-Type', kind)
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def fixture_reply(self, stream, delta, stop):
            if stream:
                chunk = {'choices': [{'delta': delta}]}
                end = {'choices': [{'delta': {}, 'finish_reason': stop}]}
                self.reply(200, 'text/event-stream', f'data: {json.dumps(chunk)}\n\ndata: {json.dumps(end)}\n\ndata: [DONE]\n\n'.encode())
            else:
                payload = {'choices': [{'message': {'role': 'assistant', **delta}, 'finish_reason': stop}],
                           'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2}}
                self.reply(200, 'application/json', json.dumps(payload).encode())

        def do_POST(self):
            nonlocal awaiting_read
            raw = self.rfile.read(int(self.headers['Content-Length']))
            request = json.loads(raw)
            messages = request['messages']
            if any(m['role'] == 'system' and isinstance(m['content'], str)
                   and m['content'].startswith('You extract durable, reusable memory') for m in messages):
                self.fixture_reply(request.get('stream'), {'content': '{"items":[]}'}, 'stop')
                return
            if request.get('tools') and awaiting_read:
                awaiting_read = False
                call = {'id': 'call_fixture_numbered_read', 'type': 'function',
                        'function': {'name': 'read', 'arguments': json.dumps(args, ensure_ascii=False)}}
                if request.get('stream'):
                    call['index'] = 0
                self.fixture_reply(request.get('stream'), {'tool_calls': [call]}, 'tool_calls')
                return
            if request.get('tools') and not captured:
                body, = [m['content'] for m in messages if m['role'] == 'tool']
                captured.append({'bytes': len(body.encode()), 'sha256': digest(body),
                                 'unpadded_sha256': unpadded_digest(body),
                                 'numbered_lines': len(re.findall(r'^ *[0-9]+\t', body, re.M)),
                                 'padded_labels': len(re.findall(r'^ +[0-9]+\t', body, re.M))})
            address, port = relay.server.server_address
            connection = HTTPConnection(address, port, timeout=75)
            try:
                connection.request('POST', self.path, raw, {'Content-Type': 'application/json'})
                response = connection.getresponse()
                self.reply(response.status, response.getheader('Content-Type'), response.read())
            finally:
                connection.close()

    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    server.url = f'http://127.0.0.1:{server.server_port}'
    Thread(target=server.serve_forever, daemon=True).start()
    try:
        yield server, captured
    finally:
        server.shutdown()
        server.server_close()


async def run(args):
    args.out.mkdir()
    relay = acp_fixture.base.Relay(os.environ.pop('DEEPSEEK_API_KEY'), args.out / 'provider-usage.jsonl', max_requests=8)
    trials = []
    try:
        for index, (name, (files, call_args, prompt, expected)) in enumerate(cases(args.shape).items()):
            order = args.order if index == 0 else args.order[::-1]
            for arm in order:
                label = f'{name}-{arm}'
                root = args.out / 'fixture'
                workspace = root / 'workspace'
                workspace.mkdir(parents=True)
                for path, content in files.items():
                    (workspace / path).write_bytes(content.encode())
                relay.take(label, 1)
                with endpoint(relay, call_args) as (server, capture):
                    async with acp_fixture.connect(getattr(args, arm), root, server, args.embedding) as (client, session):
                        response, text, _, _ = await client.call('session/prompt', {
                            'sessionId': session, 'prompt': [{'type': 'text', 'text': prompt}],
                        }, timeout=120)
                    assert await relay.settle_usage(label)
                usage = acp_fixture.base.summarise_usage([row for row in relay.rows if row['trial'] == label])
                journal, = (workspace / '.aster' / 'sessions').glob('*/*/*.jsonl')
                facts = list(map(json.loads, journal.read_text().splitlines()))
                reads = {fact['payload']['prepared']['identity']['tool_call_id'] for fact in facts
                         if fact['type'] == 'tool.prepared' and fact['payload']['prepared']['tool_name'] == 'read'}
                originals = [json.loads(fact['payload']['receipt']['output'])['text'] for fact in facts
                             if fact['type'] == 'tool.result' and fact['payload'].get('toolCallId') in reads
                             and fact['payload']['receipt']['outcome'] == 'succeeded']
                try:
                    answer = json.loads(re.sub(r'^\s*```(?:json)?\s*|\s*```\s*$', '', text))
                except json.JSONDecodeError:
                    answer = None
                stop = response.get('result', {}).get('stopReason')
                preserved = all((workspace / path).read_bytes() == content.encode() for path, content in files.items())
                wire_equivalent = (len(originals) == len(capture) == 1
                                   and unpadded_digest(f'Tool exit code: 0\n{originals[0]}') == capture[0]['unpadded_sha256'])
                passed = (stop == 'end_turn' and answer == expected and preserved and len(originals) == 1
                          and not usage['tool_calls'] and usage['usage_complete'])
                if args.shape == 'bounded':
                    passed = (passed and wire_equivalent and capture[0]['numbered_lines'] > 0
                              and capture[0]['padded_labels'] == (capture[0]['numbered_lines'] if arm == 'reference' else 0))
                trials.append({'label': label, 'case': name, 'arm': arm, 'passed': passed,
                               'answer': answer, 'expected': expected, 'capture': capture,
                               'read_args': call_args, 'files_preserved': preserved, 'wire_equivalent': wire_equivalent,
                               'raw_read_digests': [digest(x) for x in originals], **usage})
                root.rename(args.out / label)
                (args.out / 'trials.json').write_text(json.dumps(trials, ensure_ascii=False, indent=2) + '\n')
                print(label, passed, capture, usage['tokens'], flush=True)
    finally:
        relay.server.shutdown()
        relay.server.server_close()
    if args.shape == 'bounded':
        return all(trial['passed'] for trial in trials)
    return True


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for field in ('reference', 'candidate', 'embedding', 'out'):
        parser.add_argument('--' + field, type=lambda p: Path(p).resolve(), required=True)
    parser.add_argument('--order', choices=['reference', 'candidate'], nargs=2, required=True)
    parser.add_argument('--shape', choices=['large', 'bounded'], default='large')
    args = parser.parse_args()
    assert sorted(args.order) == ['candidate', 'reference']
    sys.exit(not asyncio.run(run(args)))
