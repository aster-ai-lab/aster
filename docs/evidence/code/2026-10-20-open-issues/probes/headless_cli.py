"""Bounded real CLI + ACP + loopback-provider regression; no account state."""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import threading
import time

parser = argparse.ArgumentParser()
parser.add_argument('--binaries', type=Path, required=True)
parser.add_argument('--phase', choices=['before', 'after'], required=True)
parser.add_argument('--evidence', type=Path, required=True)
args = parser.parse_args()
marker = 'HEADLESS_ISSUE2_OK'
rule_marker = 'ISSUE3_RULE_7d2c_HOST_ONLY'
requests = []

class Provider(BaseHTTPRequestHandler):
    def log_message(self, *unused):
        pass

    def do_POST(self):
        size = int(self.headers.get('Content-Length', '0'))
        assert 0 < size < 8 * 1024 * 1024
        data = json.loads(self.rfile.read(size))
        system = data.get('system', '')
        system_messages = [message.get('content') for message in data.get('messages', []) if message.get('role') == 'system']
        requests.append({'path': self.path, 'stream': bool(data.get('stream')), 'system_rule_marker_seen': rule_marker in json.dumps([system, system_messages], ensure_ascii=False)})
        text = marker
        # This fixture's sole non-streaming call is the automatic pre-analysis.
        if not data.get('stream'):
            text = json.dumps({'intent': 'GeneralPurpose', 'requires_planning': False, 'goal': {'description': 'Return the fixture marker', 'success_criteria': []}, 'execution_plan': {'complexity': 'Simple', 'steps': [{'id': 'step-1', 'description': 'Return the fixture marker', 'dependencies': [], 'success_criteria': 'Return the fixture marker'}], 'required_tools': []}, 'optimized_input': ''})
        if data.get('stream'):
            chunks = [{'id': 'issue2-fixture', 'object': 'chat.completion.chunk', 'model': 'gpt', 'choices': [{'delta': {'role': 'assistant', 'content': text}}]}, {'id': 'issue2-fixture', 'object': 'chat.completion.chunk', 'model': 'gpt', 'choices': [{'delta': {}, 'finish_reason': 'stop'}]}]
            body = (''.join('data: ' + json.dumps(chunk) + '\n\n' for chunk in chunks) + 'data: [DONE]\n\n').encode()
            content_type = 'text/event-stream'
        else:
            body = json.dumps({'id': 'issue2-fixture', 'object': 'chat.completion', 'model': 'gpt', 'choices': [{'index': 0, 'message': {'role': 'assistant', 'content': text}, 'finish_reason': 'stop'}], 'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2}}).encode()
            content_type = 'application/json'
        self.send_response(200)
        self.send_header('Content-Type', content_type)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

server = ThreadingHTTPServer(('127.0.0.1', 0), Provider)
threading.Thread(target=server.serve_forever, daemon=True).start()
args.evidence.mkdir(parents=True, exist_ok=True)
cases = [('acl_only', False, None), ('acl_and_api_key', True, None)]
if args.phase == 'after':
    cases.extend([('rules_cli', False, 'rules'), ('unsupported_override', False, 'override')])
results = []
try:
    for name, api_key, prompt_option in cases:
        with tempfile.TemporaryDirectory(prefix='aster-issue2-') as temporary:
            scope = Path(temporary)
            home, workspace = scope / 'home', scope / 'workspace'
            home.mkdir()
            workspace.mkdir()
            config = home / '.a3s/config.acl'
            config.parent.mkdir()
            config.write_text(f'providers "mock" {{\n base_url = "http://127.0.0.1:{server.server_port}/v1"\n api_key = "issue2-fixture-not-real"\n models "gpt" {{\n }}\n}}\ndefault_model = "mock/gpt"\n')
            environment = {'HOME': str(home), 'GROK_HOME': str(home / 'grok'), 'XDG_CONFIG_HOME': str(home / 'config'), 'XDG_CACHE_HOME': str(home / 'cache'), 'XDG_DATA_HOME': str(home / 'data'), 'XDG_STATE_HOME': str(home / 'state'), 'A3S_CONFIG': str(config), 'A3S_ACP_AGENT_BIN': str(args.binaries.resolve() / 'a3s-code-acp'), 'A3S_DISABLE_CC_SWITCH': '1', 'A3S_DISABLE_GROK_ACCOUNT': '1', 'A3S_DISABLE_CURSOR_ACCOUNT': '1', 'PATH': '/usr/bin:/bin:/usr/sbin:/opt/homebrew/bin', 'TERM': 'dumb', 'LANG': 'en_US.UTF-8', 'RUST_LOG': 'warn'}
            if api_key:
                environment['A3S_API_KEY'] = 'issue2-fixture-not-real'
            command = [str(args.binaries.resolve() / 'aster'), '-p', 'Return ' + marker, '--cwd', str(workspace), '--output-format', 'json']
            if prompt_option == 'rules':
                command.extend(['--rules', rule_marker])
            elif prompt_option == 'override':
                command.extend(['--system-prompt-override', rule_marker])
            before_count = len(requests)
            started = time.monotonic()
            process = subprocess.Popen(command, env=environment, cwd=workspace, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
            timed_out = False
            try:
                stdout, stderr = process.communicate(timeout=40)
            except subprocess.TimeoutExpired:
                timed_out = True
                os.killpg(process.pid, signal.SIGKILL)
                stdout, stderr = process.communicate(timeout=5)
            calls = requests[before_count:]
            result = {'case': name, 'returncode': process.returncode, 'timed_out': timed_out, 'elapsed_seconds': round(time.monotonic() - started, 3), 'provider_requests': calls, 'stdout_contains_marker': marker.encode() in stdout, 'explicit_override_rejection': b'systemPromptOverride is not supported' in stdout + stderr, 'stdout_bytes': len(stdout), 'stderr_bytes': len(stderr)}
            (args.evidence / f'headless-{args.phase}-{name}.stdout').write_bytes(stdout[:65536])
            (args.evidence / f'headless-{args.phase}-{name}.stderr').write_bytes(stderr[:65536])
            results.append(result)
            print(json.dumps(result), flush=True)
finally:
    server.shutdown()
receipt = {'phase': args.phase, 'fixture': 'isolated child home, explicit local ACL, real CLI and ACP, loopback-only provider', 'cases': results, 'binary_sha256': {name: hashlib.sha256((args.binaries / name).read_bytes()).hexdigest() for name in ['aster', 'a3s-code-acp']}}
(args.evidence / f'headless-{args.phase}-result.json').write_text(json.dumps(receipt, indent=2) + '\n')
if args.phase == 'after':
    assert all(result['returncode'] == 0 and not result['timed_out'] and result['stdout_contains_marker'] and result['provider_requests'] for result in results[:3])
    assert all(len(result['provider_requests']) == 2 and [call['stream'] for call in result['provider_requests']] == [False, True] for result in results[:3]), 'one valid pre-analysis then one main streamed answer'
    assert any(call['system_rule_marker_seen'] for call in results[2]['provider_requests'])
    assert results[3]['returncode'] != 0 and not results[3]['timed_out'] and results[3]['explicit_override_rejection'] and not results[3]['provider_requests']
