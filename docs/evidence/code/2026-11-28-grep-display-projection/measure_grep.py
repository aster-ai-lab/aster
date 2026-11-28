#!/usr/bin/env python3
"""Measure one real DeepSeek response to the same native grep page in each ACP."""

import argparse
import asyncio
from contextlib import contextmanager
import hashlib
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import sys
from threading import Thread

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / '2026-11-27-memory-relation-references'))
import compare_relations as fixture


def cases():
    routes = {
        f'config/regions/{region} configuration/production/service-routing-table.ini': ''.join(
            f'# audited-owner: {region}-{20 + n}\nroute.{region}.{n:02}=target-{n + start}\n'
            f'  mode=region-{n}\n\n' for n in range(1, 17)
        )
        for region, start in [('east', 100), ('west', 900)]
    }
    east, west = routes
    yield (
        'paged-routes', routes, {'mode': 'grep', 'query': '^route\\.', 'context': 1, 'limit': 25},
        'In the supplied search page, locate route.east.07 and route.west.06. Return a JSON object keyed '
        'by east, west, and cursor. For each route include path, line, target, owner, and mode. Use the '
        'adjacent lines as evidence for owner and mode. Copy the next cursor as a string.',
        {'east': {'path': east, 'line': 26, 'target': 'target-107', 'owner': 'east-27', 'mode': 'region-7'},
         'west': {'path': west, 'line': 22, 'target': 'target-906', 'owner': 'west-26', 'mode': 'region-6'},
         'cursor': '25'},
    )
    path = 'manual/技术 排障/发布记录 with spaces and long descriptions/rolling-audit.txt'
    records = ''.join(f'audit event {n:02} checksum GRT-{500 + n}\n' for n in range(1, 45))
    yield (
        'unicode-audit', {path: records}, {'mode': 'grep', 'query': '^audit event'},
        'Locate audit event 31 in the supplied search result. Reply with only a JSON object with keys '
        'path, line, and checksum. Give the exact source path and line.',
        {'path': path, 'line': 31, 'checksum': 'GRT-531'},
    )


@contextmanager
def endpoint(relay, tool_args):
    capture = []
    dispatched = False

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def reply(self, code, content_type, body):
            self.send_response(code)
            self.send_header('Content-Type', content_type)
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def scripted(self, stream, delta, stop):
            if stream:
                chunks = [{'choices': [{'delta': delta}]},
                          {'choices': [{'delta': {}, 'finish_reason': stop}]}]
                body = ''.join(f'data: {json.dumps(chunk)}\n\n' for chunk in chunks)
                self.reply(200, 'text/event-stream', (body + 'data: [DONE]\n\n').encode())
            else:
                payload = {'choices': [{'message': {'role': 'assistant', **delta}, 'finish_reason': stop}],
                           'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2}}
                self.reply(200, 'application/json', json.dumps(payload).encode())

        def do_POST(self):
            nonlocal dispatched
            raw = self.rfile.read(int(self.headers['Content-Length']))
            request = json.loads(raw)
            messages = request['messages']
            if any(m['role'] == 'system' and isinstance(m['content'], str)
                   and m['content'].startswith('You extract durable, reusable memory') for m in messages):
                self.scripted(request.get('stream'), {'content': '{"items":[]}'}, 'stop')
                return
            if request.get('tools') and not dispatched:
                dispatched = True
                call = {'id': 'call_fixed_native_search', 'type': 'function',
                        'function': {'name': 'search', 'arguments': json.dumps(tool_args)}}
                if request.get('stream'):
                    call['index'] = 0
                self.scripted(request.get('stream'), {'tool_calls': [call]}, 'tool_calls')
                return
            if request.get('tools') and not capture:
                capture.extend(m['content'] for m in messages if m['role'] == 'tool')
            connection = HTTPConnection(*relay.server.server_address, timeout=75)
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
        yield server, capture
    finally:
        server.shutdown()
        server.server_close()


def restore_paths(body, files):
    exit_code = 'Tool exit code: 0\n'
    legend = 'Grep results grouped by workspace-relative file path. Numbers are source lines. > marks a match.\n'
    if body.startswith(exit_code + legend):
        body = exit_code + body[len(exit_code + legend):]
    path = None
    rows = []
    headers = {f'=== {file} ===\n': file for file in files}
    for row in body.splitlines(keepends=True):
        if row in headers:
            path = headers[row]
        elif path and re.match(r'^[> ]\d+: ', row):
            rows.append(f'{row[0]}{path}:{row[1:]}')
        else:
            rows.append(row)
    return ''.join(rows)


def restore_context_paths(body, files):
    prefix = 'Tool exit code: 0\nContext lines use the file path on the matching line in the same grep block.\n'
    if not body.startswith(prefix):
        return body
    groups = re.split(r'^--\n', body[len(prefix):], flags=re.M)
    complete, footer = groups[:-1], groups[-1]
    restored = ['Tool exit code: 0\n']
    for group in complete:
        rows = group.splitlines(keepends=True)
        path, = [file for file in files if any(row.startswith(f'>{file}:') for row in rows)]
        restored.extend(f' {path}:{row[1:]}' if re.match(r'^ \d+: ', row) else row for row in rows)
        restored.append('--\n')
    return ''.join(restored) + footer


async def run(args):
    args.out.mkdir()
    relay = fixture.base.Relay(os.environ.pop('DEEPSEEK_API_KEY'), args.out / 'provider-usage.jsonl', max_requests=8)
    trials = []
    try:
        for index, (name, files, tool_args, prompt, expected) in enumerate(cases()):
            order = args.order if index == 0 else args.order[::-1]
            for arm in order:
                label = f'{name}-{arm}'
                root = args.out / label
                workspace = root / 'workspace'
                workspace.mkdir(parents=True)
                for path, content in files.items():
                    destination = workspace / path
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    destination.write_bytes(content.encode())
                relay.take(label, 1)
                with endpoint(relay, tool_args) as (server, capture):
                    async with fixture.connect(getattr(args, arm), root, server, args.embedding) as (client, session):
                        response, text, _, _ = await client.call('session/prompt', {
                            'sessionId': session, 'prompt': [{'type': 'text', 'text': prompt}],
                        }, timeout=120)
                    assert await relay.settle_usage(label)
                provider_rows = [row for row in relay.rows if row['trial'] == label]
                usage = fixture.base.summarise_usage(provider_rows)
                journal, = (workspace / '.aster' / 'sessions').glob('*/*/*.jsonl')
                facts = list(map(json.loads, journal.read_text().splitlines()))
                calls = {fact['payload']['prepared']['identity']['tool_call_id'] for fact in facts
                         if fact['type'] == 'tool.prepared' and fact['payload']['prepared']['tool_name'] == 'search'}
                observations = [json.loads(fact['payload']['receipt']['output'])['text'] for fact in facts
                                if fact['type'] == 'tool.result' and fact['payload'].get('toolCallId') in calls
                                and fact['payload']['receipt']['outcome'] == 'succeeded']
                try:
                    answer = json.loads(re.sub(r'^\s*```(?:json)?\s*|\s*```\s*$', '', text))
                except json.JSONDecodeError:
                    answer = None
                body = capture[0] if len(capture) == 1 else ''
                restore = restore_context_paths if args.projection == 'context' else restore_paths
                first_wire_equivalent = bool(observations) and restore(body, files) == f'Tool exit code: 0\n{observations[0]}'
                wire_equivalent = len(observations) == 1 and first_wire_equivalent
                first_tokens = fixture.base.summarise_usage(provider_rows[:1])['tokens']
                embedded_json = re.search(r'```json\s*(.*?)\s*```', text, flags=re.S)
                evidence = answer
                if embedded_json:
                    try:
                        evidence = json.loads(embedded_json[1])
                    except json.JSONDecodeError:
                        pass
                headers = sum(body.count(f'=== {path} ===\n') for path in files)
                has_context_legend = body.startswith('Tool exit code: 0\nContext lines use the file path')
                match_rows = [line for line in body.splitlines() if line.startswith('>')]
                match_rows_unchanged = bool(observations) and match_rows == [
                    line for line in observations[0].splitlines() if line.startswith('>')
                ]
                expected_wire_format = headers == (len(files) if arm == 'candidate' else 0)
                if args.projection == 'context':
                    expected_wire_format = (match_rows_unchanged and not headers
                                            and has_context_legend == (arm == 'candidate' and bool(tool_args.get('context'))))
                stop = response.get('result', {}).get('stopReason')
                preserved = all((workspace / path).read_bytes() == content.encode() for path, content in files.items())
                passed = (stop == 'end_turn' and answer == expected and preserved and wire_equivalent
                          and expected_wire_format
                          and not usage['tool_calls'] and usage['usage_complete'])
                trial = {'label': label, 'case': name, 'arm': arm, 'passed': passed, 'stop': stop,
                         'answer': answer, 'expected': expected, 'tool_args': tool_args, 'files_preserved': preserved,
                         'wire_equivalent': wire_equivalent, 'first_wire_equivalent': first_wire_equivalent,
                         'first_tokens': first_tokens, 'exact_json_evidence': evidence == expected,
                         'projection': args.projection, 'match_rows_unchanged': match_rows_unchanged,
                         'expected_wire_format': expected_wire_format,
                         'file_headers': headers, 'tool_bytes': len(body.encode()),
                         'raw_sha256': [hashlib.sha256(x.encode()).hexdigest() for x in observations], **usage}
                trials.append(trial)
                (args.out / 'trials.json').write_text(json.dumps(trials, ensure_ascii=False, indent=2) + '\n')
                print(json.dumps(trial, ensure_ascii=False), flush=True)
    finally:
        relay.server.shutdown()
        relay.server.server_close()
    return all(trial['passed'] for trial in trials)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for field in ('reference', 'candidate', 'embedding', 'out'):
        parser.add_argument('--' + field, type=lambda p: Path(p).resolve(), required=True)
    parser.add_argument('--order', choices=['reference', 'candidate'], nargs=2, required=True)
    parser.add_argument('--projection', choices=['file', 'context'], default='file')
    args = parser.parse_args()
    assert sorted(args.order) == ['candidate', 'reference']
    sys.exit(not asyncio.run(run(args)))
