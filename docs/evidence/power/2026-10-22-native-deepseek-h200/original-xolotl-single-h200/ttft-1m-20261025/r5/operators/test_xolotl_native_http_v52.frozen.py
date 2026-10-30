"""Replay actual native output through the authenticated real HTTP server with a CPU mock engine."""
import argparse
import json
import os
import secrets
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--source-root', required=True)
parser.add_argument('--tokenizer', required=True)
parser.add_argument('--fixture', required=True)
parser.add_argument('--out', required=True)
args = parser.parse_args()
source = Path(args.source_root)
fixture = json.loads(Path(args.fixture).read_text())
result = dict(unix=time.time(), phase='starting', cpu_only=True, public_fixture_only=True, cases=[])
process = None
key = 'cpu-test-' + secrets.token_urlsafe(24)
opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
try:
    if not fixture['public_fixture_only']:
        raise RuntimeError('HTTP fixture must be declared public')
    with tempfile.TemporaryDirectory(prefix='native-http-v52-', dir=Path(args.out).parent) as folder:
        root = Path(folder)
        config = root / 'mock-config.json'
        config.write_text(json.dumps(dict(api_monitor=False, open_browser=False, model_name='cpu-native-tool-probe',
            reasoning_effort='none', idle_unload_s=0, tool_call_recovery=False)))
        sock = socket.socket()
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
        sock.close()
        generated = fixture['raw_generated_text']
        prefix = fixture['forced_prefix']
        command = [sys.executable, '-u', str(source / 'tools/v41/serve/serve_ds41.py'), '--engine', 'mock',
            '--host', '127.0.0.1', '--port', str(port), '--tokenizer', args.tokenizer, '--config', str(config)]
        for script in [generated, generated, prefix + generated, prefix + generated]:
            command += ['--script', script]
        env = {name: value for name, value in os.environ.items() if not name.startswith(('STRATA_', 'ASTER_XQ_'))}
        env.update(CUDA_VISIBLE_DEVICES='', STRATA_API_KEY=key, STRATA_TEXT_LOG='0', PYTHONDONTWRITEBYTECODE='1')
        log = root / 'mock-server.log'
        with log.open('xb') as output:
            process = subprocess.Popen(command, cwd=source, env=env, stdin=subprocess.DEVNULL,
                stdout=output, stderr=output, start_new_session=True)
        try:
            for _ in range(100):
                if process.poll() is not None:
                    raise RuntimeError('CPU mock server exited before readiness')
                try:
                    request = urllib.request.Request('http://127.0.0.1:' + str(port) + '/health',
                        headers={'Authorization': 'Bearer ' + key})
                    with opener.open(request, timeout=1) as response:
                        if json.load(response).get('loaded'):
                            break
                except OSError:
                    pass
                time.sleep(.1)
            else:
                raise RuntimeError('CPU mock server readiness budget exhausted')
            tools = [dict(type='function', function=dict(name='add_numbers', description='Add two integers.',
                parameters=dict(type='object', properties=dict(a=dict(type='integer'), b=dict(type='integer')), required=['a', 'b'])))]
            for forced, stream in [(True, True), (True, False), (False, True), (False, False)]:
                payload = dict(model='cpu-native-tool-probe', messages=[dict(role='user',
                    content='Call add_numbers with a=17 and b=25.')], tools=tools, max_tokens=128,
                    temperature=0, seed=1, reasoning_effort='none', stream=stream)
                if forced:
                    payload['tool_choice'] = dict(type='function', function=dict(name='add_numbers'))
                if stream:
                    payload['stream_options'] = dict(include_usage=True)
                request = urllib.request.Request('http://127.0.0.1:' + str(port) + '/v1/chat/completions',
                    data=json.dumps(payload).encode(), headers={'Content-Type': 'application/json', 'Authorization': 'Bearer ' + key})
                calls = {}
                finish = None
                done = False
                with opener.open(request, timeout=15) as response:
                    if stream:
                        received = 0
                        for line in response:
                            received += len(line)
                            if received > 131072:
                                raise RuntimeError('CPU HTTP response bound exceeded')
                            if not line.startswith(b'data:'):
                                continue
                            value = line[5:].strip()
                            if value == b'[DONE]':
                                done = True
                                break
                            event = json.loads(value)
                            if event.get('error'):
                                raise RuntimeError('CPU HTTP SSE error: ' + json.dumps(event['error']))
                            for choice in event.get('choices', []):
                                finish = choice.get('finish_reason') or finish
                                for call in choice.get('delta', {}).get('tool_calls') or []:
                                    target = calls.setdefault(call.get('index', 0), dict(name='', arguments=''))
                                    function = call.get('function') or {}
                                    target['name'] += function.get('name') or ''
                                    target['arguments'] += function.get('arguments') or ''
                    else:
                        raw = response.read(131073)
                        if len(raw) > 131072:
                            raise RuntimeError('CPU HTTP response bound exceeded')
                        event = json.loads(raw)
                        if event.get('error'):
                            raise RuntimeError('CPU HTTP JSON error: ' + json.dumps(event['error']))
                        choice = event['choices'][0]
                        finish = choice['finish_reason']
                        calls = {i: call['function'] for i, call in enumerate(choice['message'].get('tool_calls') or [])}
                        done = True
                if not done or finish != 'tool_calls' or len(calls) != 1:
                    raise RuntimeError('CPU HTTP did not return one complete tool call')
                call = calls[0]
                if call['name'] != fixture['expected_name'] or json.loads(call['arguments']) != fixture['expected_arguments']:
                    raise RuntimeError('CPU HTTP changed complete native JSON arguments')
                result['cases'].append(dict(forced=forced, stream=stream, passed=True))
            result.update(phase='passed', cases_passed=len(result['cases']))
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
            result['mock_process_reaped'] = process.poll() is not None
            if result['phase'] != 'passed' and log.exists() and log.stat().st_size < 262144:
                result['mock_log_tail'] = log.read_text(errors='replace')[-3500:].replace(key, '[REDACTED-TEST-KEY]')
except BaseException as error:
    result.update(phase='failed', error=(type(error).__name__ + ': ' + str(error)).replace(key, '[REDACTED-TEST-KEY]'))
finally:
    result['finished_unix'] = time.time()
    Path(args.out).write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))
if result['phase'] != 'passed':
    raise SystemExit(1)
