#!/usr/bin/env python3
"""Run the frozen two-turn Aster token fixture over ACP with provider usage."""
import argparse
import asyncio
from collections import Counter
import hashlib
import http.client
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import threading
import time

MODEL = 'deepseek-flash'
FILES = {'alpha.txt': b'ALPHA-MARKER-8419\n', 'beta.txt': b'BETA-MARKER-2903\n',
         'gamma.txt': b'GAMMA-MARKER-6174\n'}
COMMAND = r"printf '\033[32mANSI-OK-5512\033[0m\n'"
TASK = (
    (f'Read alpha.txt and beta.txt, and run bash command `{COMMAND}`. Quote all three exact markers.',
     ('ALPHA-MARKER-8419', 'BETA-MARKER-2903', 'ANSI-OK-5512')),
    ('Now read gamma.txt and report the GAMMA marker together with the ANSI-OK marker from the previous turn.',
     ('GAMMA-MARKER-6174', 'ANSI-OK-5512')),
)
USAGE = ('prompt_tokens', 'completion_tokens', 'total_tokens', 'prompt_cache_hit_tokens', 'prompt_cache_miss_tokens')

class Relay:
    def __init__(self, credential, logfile, *, max_tokens=1024, max_requests=None):
        self.active = None
        self.lock = threading.Lock()
        self.rows = []
        self.credential = credential
        self.logfile = logfile
        self.started = Counter()
        self.pending = Counter()
        relay = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_GET(self):
                if self.path not in ('/models', '/v1/models'):
                    self.send_error(404)
                    return
                body = json.dumps({'object': 'list', 'data': [{'id': MODEL, 'object': 'model', 'owned_by': 'deepseek'}]}).encode()
                self.send_response(200)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def do_POST(self):
                if self.path not in ('/chat/completions', '/v1/chat/completions'):
                    relay.record({'rejected_path': self.path.split('?')[0]})
                    self.send_error(404)
                    return
                length = int(self.headers.get('Content-Length', 0))
                if not 0 < length <= 4 * 1024 * 1024:
                    self.send_error(413)
                    return
                raw = self.rfile.read(length)
                request = json.loads(raw)
                messages = request.get('messages') or []
                tools = request.get('tools') or []
                identity = {'requested_model': request.get('model'), 'message_count': len(messages),
                            'tool_definitions': len(tools), 'request_bytes': len(raw),
                            'requested_thinking': (request.get('thinking') or {}).get('type'),
                            'native_deferred_catalog': any(tool['function']['name'] == 'load_tool_definitions'
                                                           for tool in tools if isinstance(tool.get('function'), dict))}
                lookups = {call.get('id') for message in messages for call in message.get('tool_calls') or []
                           if (call.get('function') or {}).get('name') == 'load_tool_definitions'}
                if lookups:
                    identity['lookup_result_bytes'] = [len(message['content'].encode()) for message in messages
                                                        if message.get('role') == 'tool'
                                                        and message.get('tool_call_id') in lookups
                                                        and isinstance(message.get('content'), str)]
                if request.get('model') != MODEL:
                    relay.record({**identity, 'status': 409})
                    self.send_error(409, 'frozen model mismatch')
                    return
                with relay.lock:
                    binding = relay.active
                    if binding is not None and (max_requests is None or relay.started[binding[0]] < max_requests):
                        relay.started[binding[0]] += 1
                        relay.pending[binding[0]] += 1
                        admitted = True
                    else:
                        admitted = False
                if not admitted:
                    status = 429 if binding else 409
                    relay.record({**identity, 'status': status}, binding)
                    self.send_error(status)
                    return
                request.pop('reasoning_effort', None)
                request.pop('max_completion_tokens', None)
                request.pop('top_p', None)
                request.update(thinking={'type': 'disabled'}, max_tokens=max_tokens, temperature=0)
                if request.get('stream'):
                    request['stream_options'] = {**(request.get('stream_options') or {}), 'include_usage': True}
                wire = json.dumps(request, separators=(',', ':'), ensure_ascii=False).encode()
                details = {**identity, 'forwarded_bytes': len(wire), 'stream': bool(request.get('stream'))}
                role_bytes = Counter()
                fingerprints = []
                for message in messages:
                    message_wire = json.dumps(message, separators=(',', ':'), ensure_ascii=False).encode()
                    role_bytes[message.get('role')] += len(message_wire)
                    if tools:
                        fingerprints.append([len(message_wire), hashlib.sha256(message_wire).hexdigest()])
                details.update(role_bytes=dict(role_bytes),
                               tool_definition_bytes=len(json.dumps(tools, separators=(',', ':'), ensure_ascii=False).encode()),
                               system_message_count=sum(message.get('role') == 'system' for message in messages))
                readback = sum('[host verification state]\nThe current workspace changes are verified. '
                               'The host already read back the changed files' in message['content']
                               for message in messages if isinstance(message.get('content'), str))
                if readback:
                    details['host_disk_readback_contexts'] = readback
                recall_roles = [message.get('role') for message in messages
                                if isinstance(message.get('content'), str)
                                and ' type="Memory">\n' in message['content']]
                if recall_roles:
                    details['memory_context_message_roles'] = recall_roles
                batch_references = sum(message['content'].count(
                    '[aster host: Fresh read returned the same bytes as the preceding full batch section for this file and range.]')
                    for message in messages if message.get('role') == 'tool' and isinstance(message.get('content'), str))
                if batch_references:
                    details['batch_read_section_references'] = batch_references
                if not tools:
                    systems = [m['content'] for m in messages if m.get('role') == 'system'
                               and isinstance(m.get('content'), str)]
                    if any(body.startswith('You are a pre-analysis assistant.') for body in systems):
                        details['pre_analysis_request'] = True
                    elif any(body.startswith('You extract durable, reusable memory for a coding agent.')
                             for body in systems):
                        details['memory_extraction_request'] = True
                if fingerprints:
                    details['message_fingerprints'] = fingerprints
                packets = {}
                finish = set()
                emitted_tools = set()
                models = set()
                started = time.monotonic()
                first_byte = None
                connection = http.client.HTTPSConnection('api.deepseek.com', timeout=60)
                status = None
                error = None

                def observe(packet):
                    if packet.get('model'):
                        models.add(packet['model'])
                    usage = packet.get('usage')
                    if usage is not None:
                        key = packet.get('id', '')
                        packets[key] = {name: usage.get(name) for name in USAGE}
                        packets[key]['reasoning_tokens'] = (usage.get('completion_tokens_details') or {}).get('reasoning_tokens')
                    for choice in packet.get('choices', []):
                        if choice.get('finish_reason'):
                            finish.add(choice['finish_reason'])
                        content = choice.get('message') or choice.get('delta') or {}
                        for call in content.get('tool_calls') or []:
                            if (call.get('function') or {}).get('name'):
                                emitted_tools.add((choice.get('index', 0), call.get('index', 0), call['function']['name']))

                try:
                    connection.request('POST', '/chat/completions', body=wire,
                                       headers={'Authorization': 'Bearer ' + relay.credential,
                                                'Content-Type': 'application/json', 'Accept-Encoding': 'identity'})
                    response = connection.getresponse()
                    status = response.status
                    content_type = response.getheader('Content-Type', 'application/json')
                    self.send_response(status)
                    self.send_header('Content-Type', content_type)
                    self.send_header('Connection', 'close')
                    self.end_headers()
                    if 'text/event-stream' in content_type:
                        while line := response.readline():
                            if first_byte is None:
                                first_byte = time.monotonic()
                            if line.startswith(b'data:') and line.strip() != b'data: [DONE]':
                                observe(json.loads(line[5:]))
                            self.wfile.write(line)
                            self.wfile.flush()
                    else:
                        body = response.read()
                        first_byte = time.monotonic()
                        try:
                            packet = json.loads(body)
                            if status == 200:
                                observe(packet)
                            else:
                                error = (packet.get('error') or {}).get('code')
                        except ValueError:
                            pass
                        self.wfile.write(body)
                except (OSError, http.client.HTTPException, ValueError) as exc:
                    error = type(exc).__name__
                    if status is None:
                        self.send_error(502)
                finally:
                    connection.close()
                    relay.record({**details, 'status': status, 'error': error,
                                  'models': sorted(models), 'finish': sorted(finish),
                                  'tool_calls': [item[2] for item in sorted(emitted_tools)],
                                  'usage': list(packets.values()),
                                  'ttfb_ms': round((first_byte - started) * 1000) if first_byte is not None else None,
                                  'upstream_ms': round((time.monotonic() - started) * 1000)}, binding, completed=True)

        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        self.url = f'http://127.0.0.1:{self.server.server_port}'

    def record(self, row, binding=None, *, completed=False):
        with self.lock:
            trial, turn = binding or self.active or (None, None)
            row = {'trial': trial, 'turn': turn, **row}
            self.rows.append(row)
            with self.logfile.open('a', encoding='utf-8') as f:
                f.write(json.dumps(row, ensure_ascii=False) + '\n')
            if completed:
                self.pending[trial] -= 1

    def take(self, label, turn):
        with self.lock:
            self.active = (label, turn)

    async def settle_usage(self, label, *, timeout=20):
        async def settled():
            previous = None
            since = time.monotonic()
            while True:
                with self.lock:
                    current = self.started[label], self.pending[label]
                now = time.monotonic()
                if current != previous:
                    previous, since = current, now
                if current[1] == 0 and now - since >= 1:
                    return True
                await asyncio.sleep(0.05)

        try:
            return await asyncio.wait_for(settled(), timeout)
        except asyncio.TimeoutError:
            return False


def safe_permission(params, workspace):
    call = params.get('toolCall') or {}
    raw = call.get('rawInput') or {}
    if not isinstance(raw, dict):
        raw = {}
    kind = call.get('kind')
    command = raw.get('command') or raw.get('cmd')
    if command == COMMAND:
        return True
    location = raw.get('file_path') or raw.get('path') or raw.get('filePath')
    return (kind in ('read', 'search') and isinstance(location, str)
            and (workspace / location).resolve() in {workspace / name for name in FILES})


class Acp:
    def __init__(self, child, workspace):
        self.child = child
        self.workspace = workspace
        self.serial = 0

    async def send(self, packet):
        self.child.stdin.write((json.dumps({'jsonrpc': '2.0', **packet}) + '\n').encode())
        await self.child.stdin.drain()

    def permit(self, info):
        return safe_permission(info, self.workspace)

    def observe(self, packet):
        pass

    async def call(self, method, params, timeout=240):
        self.serial += 1
        identity = self.serial
        await self.send({'id': identity, 'method': method, 'params': params})
        text, permissions, tools = [], [], {}
        async def receive():
            while line := await self.child.stdout.readline():
                packet = json.loads(line)
                self.observe(packet)
                method = packet.get('method')
                if method == 'session/request_permission':
                    info = packet['params']
                    allow = next((option['optionId'] for option in info['options']
                                  if option.get('kind') == 'allow_once'), None)
                    permitted = bool(allow and self.permit(info))
                    call = info.get('toolCall') or {}
                    permissions.append({'kind': call.get('kind'), 'allowed': permitted})
                    outcome = {'outcome': 'selected', 'optionId': allow} if permitted else {'outcome': 'cancelled'}
                    await self.send({'id': packet['id'], 'result': {'outcome': outcome}})
                elif method == 'session/update':
                    event = packet['params']['update']
                    variant = event.get('sessionUpdate')
                    if variant == 'agent_message_chunk':
                        text.append((event.get('content') or {}).get('text', ''))
                    elif variant in ('tool_call', 'tool_call_update') and event.get('toolCallId'):
                        saved = tools.setdefault(event['toolCallId'], {})
                        saved.update({k: event[k] for k in ('kind', 'status') if k in event})
                elif method and 'id' in packet:
                    await self.send({'id': packet['id'], 'error': {'code': -32601, 'message': 'Unsupported client method'}})
                elif packet.get('id') == identity:
                    return packet
            raise RuntimeError('ACP EOF')
        packet = await asyncio.wait_for(receive(), timeout)
        return packet, ''.join(text), permissions, list(tools.values())


def acp_process(arm, binary, directory, workspace, home, relay, embedding, *, max_tokens=1024):
    env = {'PATH': os.environ['PATH'], 'HOME': str(home), 'TERM': 'xterm-256color', 'LANG': 'en_US.UTF-8'}
    if arm == 'dsh':
        dsh_home = directory / 'dsh-home'
        dsh_home.mkdir()
        patch = directory / 'relay.patch.yml'
        patch.write_text(f'''- id: llm-pi-ai
  config:
    providers:
      benchmark:
        api: openai-completions
        apiKeyEnv: ASTER_BENCHMARK_RELAY_KEY
        baseURL: {relay.url}/v1
        models:
          - id: {MODEL}
            name: DeepSeek Flash
            contextWindow: 1000000
            maxTokens: {max_tokens}
            reasoningEfforts: false
- id: acp
  config:
    provider: benchmark
    model: {MODEL}
''')
        env.update(DSH_HOME=str(dsh_home), ASTER_BENCHMARK_RELAY_KEY='local-test-key')
        command = [str(binary), '--profile', 'acp', '--patch', str(patch)]
    else:
        config = directory / 'config.acl'
        config.write_text(f'default_model = "deepseek/{MODEL}"\nreasoning_effort = "none"\n'
                          f'providers "deepseek" {{\n  api_key = "local-test-key"\n  base_url = "{relay.url}"\n'
                          f'  models "{MODEL}" {{\n    name = "DeepSeek Flash"\n'
                          '    reasoning_control = { mode = "effort", levels = ["none"] }\n  }\n}\n')
        env.update(ASTER_CONFIG=str(config), ASTER_DISABLE_GROK_ACCOUNT='1', ASTER_DISABLE_CURSOR_ACCOUNT='1')
        if embedding is not None:
            env['ASTER_EMBEDDING_DIR'] = str(embedding)
        command = [str(binary), '--cwd', str(workspace), '--model', f'deepseek/{MODEL}']
    return command, env


async def close_acp(child):
    child.stdin.close()
    try:
        await asyncio.wait_for(child.wait(), 4)
    except asyncio.TimeoutError:
        child.terminate()
        try:
            await asyncio.wait_for(child.wait(), 3)
        except asyncio.TimeoutError:
            child.kill()
            await child.wait()


def summarise_usage(rows):
    usage = [counter for row in rows for counter in row.get('usage', [])]
    complete = bool(rows) and all(row.get('status') == 200 and len(row.get('usage', [])) == 1
                                  and row.get('models') == [MODEL] for row in rows)
    complete &= all(
        all(isinstance(item[key], int) for key in USAGE)
        and item['prompt_tokens'] + item['completion_tokens'] == item['total_tokens']
        and item['prompt_cache_hit_tokens'] + item['prompt_cache_miss_tokens'] == item['prompt_tokens']
        for item in usage
    )
    return {
        'api_requests': len(rows),
        'usage_complete': complete,
        'tokens': {key: sum(item[key] for item in usage) if usage and all(isinstance(item[key], int) for item in usage) else None
                   for key in USAGE},
        'tool_calls': dict(Counter(tool for row in rows for tool in row.get('tool_calls', []))),
        'primary_requests': sum(bool(row.get('tool_definitions')) for row in rows),
    }


async def run_trial(label, arm, binary, out, relay, embedding):
    directory = out / label
    directory.mkdir()
    workspace = out / 'workspace'
    workspace.mkdir()
    home = out / 'home'
    home.mkdir()
    for name, data in FILES.items():
        (workspace / name).write_bytes(data)
    command, env = acp_process(arm, binary, directory, workspace, home, relay, embedding)
    relay.take(label, 0)
    started = time.monotonic()
    result = {'label': label, 'arm': arm, 'turns': []}
    with (directory / 'stderr.log').open('wb') as stderr:
        child = await asyncio.create_subprocess_exec(*command, cwd=workspace, env=env, stdin=asyncio.subprocess.PIPE,
                                                     stdout=asyncio.subprocess.PIPE, stderr=stderr, limit=2**22)
    client = Acp(child, workspace)
    try:
        initialize, *_ = await client.call('initialize', {'protocolVersion': 1, 'clientCapabilities': {}}, 45)
        if 'result' not in initialize:
            raise RuntimeError(f'ACP initialize {initialize["error"]["code"]}')
        opened, *_ = await client.call('session/new', {'cwd': str(workspace), 'mcpServers': []}, 45)
        if 'result' not in opened:
            raise RuntimeError(f'ACP new {opened["error"]["code"]}')
        state = opened['result']
        result['selected_model'] = (state.get('models') or {}).get('currentModelId')
        result['model_options'] = [o.get('currentValue') for o in state.get('configOptions') or [] if o.get('id') == 'model']
        result['startup_ms'] = round((time.monotonic() - started) * 1000)
        for index, (prompt, markers) in enumerate(TASK, 1):
            relay.take(label, index)
            begin = time.monotonic()
            packet, text, permissions, tools = await client.call('session/prompt', {'sessionId': state['sessionId'],
                                                                                  'prompt': [{'type': 'text', 'text': prompt}]})
            elapsed = round((time.monotonic() - begin) * 1000)
            stop = (packet.get('result') or {}).get('stopReason')
            if stop is None and 'error' in packet:
                error = packet['error']
                (directory / f'error-{index}.json').write_text(json.dumps({'code': error.get('code'), 'message': error.get('message')}))
            missing = [marker for marker in markers if marker not in text]
            (directory / f'answer-{index}.txt').write_text(text)
            result['turns'].append({'turn': index, 'elapsed_ms': elapsed, 'stop': stop, 'missing_markers': missing,
                                    'tools': dict(Counter(f"{tool.get('kind', 'other')}:{tool.get('status', 'unknown')}" for tool in tools)),
                                    'permissions': permissions})
            if stop != 'end_turn':
                break
    except (RuntimeError, OSError, asyncio.TimeoutError, ValueError) as exc:
        result['runner_error'] = str(exc) if type(exc) is RuntimeError else type(exc).__name__
    finally:
        await close_acp(child)
    result['fixtures_unchanged'] = all((workspace / name).is_file() and (workspace / name).read_bytes() == data
                                      for name, data in FILES.items())
    result['task_pass'] = (len(result['turns']) == 2 and result['fixtures_unchanged']
                           and all(t['stop'] == 'end_turn' and not t['missing_markers'] for t in result['turns']))
    if arm != 'dsh':
        receipts = []
        for log in (workspace / '.aster' / 'sessions').glob('*/*/*.jsonl'):
            for line in log.read_text().splitlines():
                fact = json.loads(line)
                receipt = fact.get('payload', {}).get('receipt')
                if fact.get('type') == 'tool.result' and isinstance(receipt, dict):
                    receipts.append(receipt)
        result['aster_receipts'] = len(receipts)
        result['aster_receipts_ansi_free'] = bool(receipts) and all('\x1b' not in str(r.get('output')) for r in receipts)
        result['task_pass'] &= result['aster_receipts_ansi_free']
    workspace.rename(directory / 'workspace')
    home.rename(directory / 'home')
    await asyncio.sleep(0.2)
    rows = [r for r in relay.rows if r['trial'] == label]
    result.update(summarise_usage(rows))
    return result


async def evaluate(args):
    args.out.mkdir()
    relay = Relay(os.environ.pop('DEEPSEEK_API_KEY'), args.out / 'provider-usage.jsonl')
    manifest = {'model': MODEL, 'endpoint': 'https://api.deepseek.com/chat/completions', 'controls': {'thinking': 'disabled',
                'temperature': 0, 'max_tokens': 1024}, 'prompts': [prompt for prompt, _ in TASK],
                'expected_markers': [markers for _, markers in TASK],
                'fixtures_sha256': {name: hashlib.sha256(data).hexdigest() for name, data in FILES.items()},
                'orders': args.order}
    (args.out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    results = []
    try:
        for index, order in enumerate(args.order, 1):
            for arm in order.split(','):
                result = await run_trial(f'{index}-{arm}', arm, getattr(args, arm), args.out, relay, args.embedding)
                results.append(result)
                (args.out / 'trials.json').write_text(json.dumps(results, indent=2) + '\n')
                print(json.dumps(result), flush=True)
    finally:
        relay.server.shutdown()
        relay.server.server_close()


def main():
    parser = argparse.ArgumentParser()
    for option in ('release', 'candidate', 'dsh', 'out', 'embedding'):
        parser.add_argument('--' + option, type=lambda value: Path(value).resolve())
    parser.add_argument('--order', nargs='+', required=True)
    args = parser.parse_args()
    asyncio.run(evaluate(args))


if __name__ == '__main__':
    main()
