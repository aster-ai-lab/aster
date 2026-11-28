#!/usr/bin/env python3
"""Compare a real continuation after model rounds that reread covered bytes."""

import argparse
import asyncio
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import sys
from threading import Thread

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / '2026-11-27-six-turn-dsh-comparison'))
import revisions

PRELUDE = (
    ({'file_path': 'README.md'}, {'file_path': 'order.json'}, {'file_path': 'policy.json'}),
    ({'file_path': 'order.json', 'byte_offset': 0},),
    ({'file_path': 'order.json', 'byte_offset': 60},),
)


class ReadReplay:
    def __init__(self, relay):
        self.relay = relay
        self.rows = relay.rows
        self.rounds = {}
        self.first_live_request = {}
        self.active = None
        replay = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def reply(self, code, content_type, body):
                self.send_response(code)
                self.send_header('Content-Type', content_type)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def replay_tools(self, round_index, arguments, stream):
                calls = [{'id': f'call_replay_{round_index}_{i}', 'type': 'function',
                          'function': {'name': 'read', 'arguments': json.dumps(args)}}
                         for i, args in enumerate(arguments)]
                if stream:
                    for index, call in enumerate(calls):
                        call['index'] = index
                    delta = {'choices': [{'delta': {'tool_calls': calls}}]}
                    stop = {'choices': [{'delta': {}, 'finish_reason': 'tool_calls'}]}
                    self.reply(200, 'text/event-stream',
                               f'data: {json.dumps(delta)}\n\ndata: {json.dumps(stop)}\n\ndata: [DONE]\n\n'.encode())
                else:
                    body = {'id': 'read-replay', 'object': 'chat.completion',
                            'choices': [{'index': 0, 'message': {'role': 'assistant', 'tool_calls': calls},
                                         'finish_reason': 'tool_calls'}],
                            'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2}}
                    self.reply(200, 'application/json', json.dumps(body).encode())

            def do_POST(self):
                raw = self.rfile.read(int(self.headers['Content-Length']))
                request = json.loads(raw)
                label, turn = replay.active
                if turn == 1 and request.get('tools'):
                    index = replay.rounds.get(label, 0)
                    if index < len(PRELUDE):
                        replay.rounds[label] = index + 1
                        self.replay_tools(index, PRELUDE[index], request.get('stream'))
                        return
                    if label not in replay.first_live_request:
                        data = [m['content'] for m in request['messages'] if m.get('role') == 'tool']
                        replay.first_live_request[label] = {
                            'tool_result_count': len(data),
                            'covered_ranges': sum('This range adds no new text' in text for text in data),
                            'recovery_notes': sum('Two consecutive tool rounds' in text for text in data),
                        }
                address, port = relay.server.server_address
                conn = HTTPConnection(address, port, timeout=75)
                try:
                    conn.request('POST', self.path, raw, {'Content-Type': 'application/json'})
                    reply = conn.getresponse()
                    self.reply(reply.status, reply.getheader('Content-Type'), reply.read())
                finally:
                    conn.close()

        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.url = f'http://127.0.0.1:{self.server.server_port}'
        Thread(target=self.server.serve_forever, daemon=True).start()

    def take(self, label, turn):
        self.active = label, turn
        self.relay.take(label, turn)

    async def settle_usage(self, label):
        return await self.relay.settle_usage(label)

    def close(self):
        self.server.shutdown()
        self.server.server_close()
        self.relay.server.shutdown()
        self.relay.server.server_close()


async def compare(args):
    args.out.mkdir()
    fixture = json.loads(args.fixture.read_text())
    assert len(fixture['turns']) == 6
    assert sorted(args.order) == ['candidate', 'release']
    (args.out / 'manifest.json').write_text(json.dumps({
        'fixture': fixture, 'scripted_first_turn_reads': PRELUDE, 'order': args.order,
        'model': revisions.base.MODEL, 'temperature': 0, 'thinking': 'disabled',
        'max_tokens': revisions.MAX_TOKENS, 'max_requests': revisions.MAX_REQUESTS,
    }, indent=2) + '\n')
    relay = ReadReplay(revisions.base.Relay(os.environ.pop('DEEPSEEK_API_KEY'), args.out / 'provider-usage.jsonl',
                                           max_tokens=revisions.MAX_TOKENS, max_requests=revisions.MAX_REQUESTS))
    results = []
    try:
        for arm in args.order:
            path = args.candidate if arm == 'candidate' else args.reference
            launcher = revisions.candidate_with_catalog(args.out / f'{arm}-launcher', path)
            trial = await revisions.run_trial(arm, arm, launcher, args, fixture, relay)
            trial['scripted_rounds'] = relay.rounds.get(arm, 0)
            trial['first_live_request'] = relay.first_live_request.get(arm)
            decisions = [r for r in relay.rows if r['trial'] == arm and r['turn'] == 1
                         and r.get('tool_definitions')]
            trial['first_turn_tool_choices'] = [r.get('tool_calls', []) for r in decisions]
            results.append(trial)
            (args.out / 'trials.json').write_text(json.dumps(results, indent=2) + '\n')
            print(arm, trial['passed'], trial['first_live_request'], trial['first_turn_tool_choices'], flush=True)
    finally:
        relay.close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ('reference', 'candidate', 'embedding', 'out', 'fixture'):
        parser.add_argument('--' + option, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--order', choices=['candidate', 'release'], nargs=2, required=True)
    asyncio.run(compare(parser.parse_args()))
