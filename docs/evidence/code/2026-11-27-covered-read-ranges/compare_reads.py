#!/usr/bin/env python3
"""Measure real model continuation after two controlled native reads through ACP."""

import argparse
import asyncio
from contextlib import contextmanager
from dataclasses import dataclass
from hashlib import sha256
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import sys
from threading import Thread

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / '2026-11-27-memory-relation-references'))
import compare_relations as acp_fixture

FILE = '装置 log.txt'
PREFIX = 'OPENING=CEDAR-2048'
TAIL = 'FINAL=FALLOW-9158'
REVISED = 'FINAL=CLOVER-4071'
LONG_TAIL = 'FINAL=ORBIT-5883'
ROWS = '\n'.join(f'{i:02d}: gauge observation in calibration log {i * 27}' for i in range(1, 42))
NORMAL = f'{PREFIX}\n{ROWS}\n{TAIL}'


@dataclass(frozen=True)
class Scenario:
    content: str
    second_read: dict
    required: tuple
    replacement: str | None = None


CASES = {
    'covered': Scenario(NORMAL, {'offset': 1}, (PREFIX, TAIL)),
    'changed': Scenario(NORMAL, {'offset': 1}, (PREFIX, REVISED), NORMAL.replace(TAIL, REVISED)),
    'continuation': Scenario(f'{PREFIX} {"long calibration " * 185}{LONG_TAIL}',
                             {'byte_offset': 2000}, (PREFIX, LONG_TAIL)),
}


@contextmanager
def endpoint(relay, source, scenario):
    initial = [{'file_path': FILE}, {'file_path': FILE, **scenario.second_read}]
    observations = []

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def reply(self, status, content_type, body):
            self.send_response(status)
            self.send_header('Content-Type', content_type)
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def fixture_reply(self, request, delta, stop):
            if request.get('stream'):
                chunk = {'choices': [{'delta': delta}]}
                end = {'choices': [{'delta': {}, 'finish_reason': stop}]}
                raw = f'data: {json.dumps(chunk)}\n\ndata: {json.dumps(end)}\n\ndata: [DONE]\n\n'
                self.reply(200, 'text/event-stream', raw.encode())
            else:
                message = {'role': 'assistant', **delta}
                raw = {'id': 'cmpl-scripted-read', 'object': 'chat.completion',
                       'choices': [{'index': 0, 'message': message, 'finish_reason': stop}],
                       'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2}}
                self.reply(200, 'application/json', json.dumps(raw).encode())

        def do_POST(self):
            raw = self.rfile.read(int(self.headers['Content-Length']))
            request = json.loads(raw)
            messages = request['messages']
            if any(m.get('role') == 'system' and isinstance(m['content'], str)
                   and m['content'].startswith('You extract durable, reusable memory') for m in messages):
                self.fixture_reply(request, {'content': '{"items":[]}'}, 'stop')
                return
            if request.get('tools') and initial:
                if len(initial) == 1 and scenario.replacement is not None:
                    source.write_bytes(scenario.replacement.encode())
                index = 3 - len(initial)
                args = initial.pop(0)
                call = {'id': f'call_fixture_read_{index}', 'type': 'function',
                        'function': {'name': 'read', 'arguments': json.dumps(args, ensure_ascii=False)}}
                if request.get('stream'):
                    call['index'] = 0
                self.fixture_reply(request, {'tool_calls': [call]}, 'tool_calls')
                return
            if request.get('tools'):
                results = [m['content'] for m in messages if m.get('role') == 'tool']
                observations.append({
                    'tool_text_bytes': [len(text.encode()) for text in results],
                    'tool_text_sha256': [sha256(text.encode()).hexdigest() for text in results],
                    'covered_read_notes': sum('This range adds no new text' in text for text in results),
                    'visible_markers': {marker: any(marker in text for text in results)
                                        for marker in scenario.required},
                })
            host, port = relay.server.server_address
            connection = HTTPConnection(host, port, timeout=75)
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
        yield server, observations
    finally:
        server.shutdown()
        server.server_close()


async def run(args):
    args.out.mkdir()
    relay = acp_fixture.base.Relay(os.environ.pop('DEEPSEEK_API_KEY'), args.out / 'provider-usage.jsonl', max_requests=8)
    acp_fixture.base.FILES = {FILE: b''}
    trials = []
    try:
        for index, (case, scenario) in enumerate(CASES.items()):
            order = ('reference', 'candidate') if index % 2 == 0 else ('candidate', 'reference')
            for arm in order:
                label = f'{case}-{arm}'
                root = args.out / label
                (root / 'workspace').mkdir(parents=True)
                source = root / 'workspace' / FILE
                source.write_bytes(scenario.content.encode())
                relay.take(label, 1)
                with endpoint(relay, source, scenario) as (server, observations):
                    async with acp_fixture.connect(getattr(args, arm), root, server, args.embedding) as (client, session):
                        response, text, permissions, _ = await client.call('session/prompt', {
                            'sessionId': session, 'prompt': [{'type': 'text', 'text': (
                                f'Read {FILE} and return the exact OPENING and current FINAL entries. '
                                'Use native read for file access. Preserve the file.'
                            )}]
                        }, timeout=120)
                    assert await relay.settle_usage(label)
                usage = acp_fixture.base.summarise_usage([row for row in relay.rows if row['trial'] == label])
                stop = response.get('result', {}).get('stopReason')
                passed = (stop == 'end_turn' and all(marker in text for marker in scenario.required)
                          and (scenario.replacement is None or TAIL not in text)
                          and source.read_bytes() == (scenario.replacement or scenario.content).encode()
                          and usage['usage_complete'])
                trial = {'label': label, 'case': case, 'arm': arm, 'passed': passed, 'stop': stop,
                         'answer': text, 'permissions': permissions, 'observations': observations, **usage}
                trials.append(trial)
                (args.out / 'trials.json').write_text(json.dumps(trials, indent=2, ensure_ascii=False) + '\n')
                print(label, passed, usage['tokens'], flush=True)
    finally:
        relay.server.shutdown()
        relay.server.server_close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ('reference', 'candidate', 'embedding', 'out'):
        parser.add_argument('--' + option, type=lambda value: Path(value).resolve(), required=True)
    asyncio.run(run(parser.parse_args()))
