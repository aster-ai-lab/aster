#!/usr/bin/env python3
"""Loopback model fixture: real native ACP/tools must create the compaction pressure."""
import argparse
import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


class Scenario:
    def __init__(self, workspace, delay=12):
        self.workspace = workspace
        self.delay = delay
        self.results = set()
        self.folds = 0
        self.lock = threading.Lock()

    def decide(self, body):
        raw = json.dumps(body, ensure_ascii=False)
        if 'You are a context-compaction engine.' in raw:
            self.folds += 1
            time.sleep(self.delay)
            return ({'role': 'assistant', 'content':
                     'CONTEXT-CUA-HANDOFF: Preserve goal CONTEXT-CUA-42; read all eight pressure files. '
                     f'Completed read count: {len(self.results)}. Continue with the next unread file.'},
                    'stop', 700, {'kind': 'compaction', 'cycle': self.folds, 'reads': len(self.results)})
        if 'You extract durable, reusable memory' in raw:
            return {'role': 'assistant', 'content': '{"items":[]}'}, 'stop', 100, {'kind': 'memory'}
        if 'emit_pre_analysis' in raw or 'You are a pre-analysis assistant' in raw:
            value = {'intent': 'GeneralPurpose', 'requires_planning': False,
                     'goal': {'description': 'CONTEXT-CUA-42', 'success_criteria': []},
                     'execution_plan': {'complexity': 'Simple', 'steps': [], 'required_tools': []},
                     'optimized_input': ''}
            return {'role': 'assistant', 'content': json.dumps(value)}, 'stop', 100, {'kind': 'pre_analysis'}
        if 'ACP-RECOVERY-42' in raw:
            return {'role': 'assistant', 'content': 'ACP-RECOVERED-42'}, 'stop', 24000, {'kind': 'recovered'}
        if 'ACP-CRASH-42' in raw:
            return {'role': 'assistant', 'content': 'ACP-HELD-42'}, 'stop', 24000, {'kind': 'held_request'}
        if 'CONTEXT-CUA-42' not in raw:
            raise ValueError('missing synthetic scenario marker')
        # The fact-log adapter may carry tool results inside canonical user messages.
        # These exact file-content markers are absent from prompts and tool arguments.
        for index in range(8):
            if f'PRESSURE-{index:02d} line ' in raw:
                self.results.add(index)
        for message in body.get('messages', []):
            call = message.get('tool_call_id', '')
            if message.get('role') == 'tool' and call.startswith('call-context-read-'):
                index = int(call.removeprefix('call-context-read-'))
                if f'PRESSURE-{index:02d}' not in str(message.get('content', '')):
                    raise ValueError('read tool did not return its owned synthetic file')
                self.results.add(index)
        index = len(self.results)
        if index >= 8:
            if self.folds < 2:
                raise ValueError('eight real reads did not trigger two actual compactions')
            if 'CONTEXT-CUA-HANDOFF' not in raw:
                raise ValueError('the next model prompt lost the actual compaction handoff')
            return ({'role': 'assistant', 'content': 'CONTEXT-CUA-42: 8 files read; handoff retained; task continued.'},
                    'stop', 24000, {'kind': 'continued', 'reads': index, 'folds': self.folds})
        offered = any(t.get('function', {}).get('name') == 'read' for t in body.get('tools', []))
        if not offered:
            raise ValueError('native read tool was not offered')
        call = {'id': f'call-context-read-{index}', 'type': 'function',
                'function': {'name': 'read', 'arguments': json.dumps({
                    'file_path': str(self.workspace / f'pressure-{index:02d}.txt'), 'limit': 300})}}
        # Controlled usage belongs to the fixture lane; it is never presented as real-provider evidence.
        tokens = 121600 if index >= 2 else 95200
        return ({'role': 'assistant', 'content': None, 'tool_calls': [call]},
                'tool_calls', tokens, {'kind': 'read_requested', 'index': index, 'reported_prompt_tokens': tokens})


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        try:
            size = int(self.headers.get('Content-Length', 0))
            if not 0 < size <= 2_000_000:
                self.send_error(413)
                return
            body = json.loads(self.rfile.read(size))
            if self.path == '/v1/systemone':
                payload = json.dumps({'model': 'context-fixture', 'answers': {'surface': {'type': 'choice', 'choice': 'prose'}}})
                content_type = 'application/json'
            else:
                with self.server.scenario.lock:
                    message, finish, tokens, event = self.server.scenario.decide(body)
                    with self.server.events.open('a') as output:
                        output.write(json.dumps({'at': time.time(), **event}) + '\n')
                if event['kind'] == 'held_request':
                    # A replacement process can call the fixture while the old request remains held.
                    time.sleep(self.server.scenario.delay)
                usage = {'prompt_tokens': tokens, 'completion_tokens': 20, 'total_tokens': tokens + 20}
                if body.get('stream'):
                    delta = {k: v for k, v in message.items() if k != 'role'}
                    for index, call in enumerate(delta.get('tool_calls', [])):
                        call['index'] = index
                    chunks = [{'choices': [{'index': 0, 'delta': delta}]},
                              {'choices': [{'index': 0, 'delta': {}, 'finish_reason': finish}], 'usage': usage}]
                    payload = ''.join('data: ' + json.dumps(c) + '\n\n' for c in chunks) + 'data: [DONE]\n\n'
                    content_type = 'text/event-stream'
                else:
                    payload = json.dumps({'id': 'context-cua', 'object': 'chat.completion', 'usage': usage,
                                          'choices': [{'index': 0, 'message': message, 'finish_reason': finish}]})
                    content_type = 'application/json'
            data = payload.encode()
            self.send_response(200)
            self.send_header('Content-Type', content_type)
            self.send_header('Content-Length', str(len(data)))
            self.end_headers()
            self.wfile.write(data)
        except (ValueError, TypeError, KeyError) as error:
            with self.server.events.open('a') as output:
                output.write(json.dumps({'kind': 'fixture_error', 'error': str(error)}) + '\n')
            self.send_error(400, 'invalid fixture request')


def self_check():
    scenario = Scenario(Path('/owned'), delay=0)
    answer = scenario.decide({'messages': [{'role': 'user', 'content': 'ACP-RECOVERY-42'}]})
    assert answer[0]['content'] == 'ACP-RECOVERED-42' and answer[3]['kind'] == 'recovered'
    assert scenario.decide({'messages': [{'role': 'user', 'content': 'ACP-CRASH-42'}]})[3]['kind'] == 'held_request'
    body = {'messages': [{'role': 'user', 'content': 'CONTEXT-CUA-42'}],
            'tools': [{'function': {'name': 'read'}}]}
    message, finish, tokens, _ = scenario.decide(body)
    assert finish == 'tool_calls' and tokens == 95200
    assert json.loads(message['tool_calls'][0]['function']['arguments'])['file_path'] == '/owned/pressure-00.txt'
    canonical = Scenario(Path('/owned'), delay=0)
    canonical.decide({**body, 'messages': body['messages'] + [{'role': 'user', 'content': 'tool result: PRESSURE-00 line 000'}]})
    assert canonical.results == {0}
    bad = {**body, 'messages': body['messages'] + [{'role': 'tool', 'tool_call_id': 'call-context-read-0', 'content': 'wrong'}]}
    try:
        scenario.decide(bad)
    except ValueError:
        pass
    else:
        raise AssertionError('foreign tool output was accepted')
    for index in range(8):
        body['messages'].append({'role': 'tool', 'tool_call_id': f'call-context-read-{index}', 'content': f'PRESSURE-{index:02d}'})
        if index == 2 or index == 5:
            scenario.decide({'messages': [{'content': 'You are a context-compaction engine.'}]})
        if index == 7:
            body['messages'].append({'role': 'system', 'content': 'CONTEXT-CUA-HANDOFF'})
        message, finish, _, _ = scenario.decide(body)
    assert finish == 'stop' and 'task continued' in message['content'] and scenario.folds == 2


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--self-check', action='store_true')
    parser.add_argument('--workspace', type=Path)
    parser.add_argument('--events', type=Path)
    parser.add_argument('--port', type=int, default=0)
    parser.add_argument('--delay', type=float, default=12)
    args = parser.parse_args()
    if args.self_check:
        self_check()
    else:
        if not args.workspace or not args.events:
            parser.error('--workspace and --events are required')
        args.events.parent.mkdir(parents=True, exist_ok=True)
        server = ThreadingHTTPServer(('127.0.0.1', args.port), Handler)
        server.events = args.events
        server.scenario = Scenario(args.workspace.resolve(), args.delay)
        print(json.dumps({'baseUrl': f'http://127.0.0.1:{server.server_port}/v1'}), flush=True)
        try:
            server.serve_forever()
        finally:
            server.server_close()
