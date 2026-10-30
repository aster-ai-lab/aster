#!/usr/bin/env python3
"""Check cancel-and-send through the actual terminal, ACP and model stream."""
import hashlib
import json
import os
from pathlib import Path
import select
import socket
import subprocess
import sys
import tempfile
import threading
from http.server import ThreadingHTTPServer

from json_render_model import Handler, decide as housekeeping

REPO = Path(__file__).resolve().parents[2]
FIRST = 'STREAM-CONTROL-FIRST'
SECOND = 'STREAM-CONTROL-SECOND'
READY = 'CONTROL STREAM READY'
DONE = 'CONTROL SECOND COMPLETE'


def decide(body):
    raw = json.dumps(body)
    if 'emit_pre_analysis' in raw or 'You are a pre-analysis assistant' in raw or 'You extract durable, reusable memory' in raw:
        return housekeeping(body)
    user = next((m.get('content', '') for m in reversed(body.get('messages', [])) if m.get('role') == 'user'), '')
    if user not in [FIRST, SECOND]:
        raise ValueError('exact stream-control request required')
    return {'role': 'assistant', 'content': READY if user == FIRST else DONE}, 'stop', {'kind': 'first' if user == FIRST else 'second'}


class StreamHandler(Handler):
    model_name = 'stream-control'
    decide = staticmethod(decide)

    def send(self, payload, content_type='application/json'):
        if content_type != 'text/event-stream' or READY not in payload:
            return super().send(payload, content_type)
        self.send_response(200)
        self.send_header('Content-Type', content_type)
        self.send_header('Connection', 'close')
        self.end_headers()
        self.close_connection = True
        delta = {'choices': [{'index': 0, 'delta': {'content': READY}}]}
        kind = 'first-ended-by-cleanup'
        try:
            self.wfile.write(('data: ' + json.dumps(delta) + '\n\n').encode())
            self.wfile.flush()
            # Keep the model turn active until its caller cancels the transport.
            while not self.server.stop.is_set():
                readable, _, _ = select.select([self.connection], [], [], 0.1)
                if readable and not self.connection.recv(1, socket.MSG_PEEK):
                    kind = 'first-transport-closed'
                    break
                self.wfile.write(b'data: {"choices":[{"index":0,"delta":{}}]}\n\n')
                self.wfile.flush()
        except OSError:
            kind = 'first-transport-closed'
        finally:
            with self.server.events.open('a') as output:
                output.write(json.dumps({'kind': kind}) + '\n')


def main():
    for request, expected in [(FIRST, READY), (SECOND, DONE)]:
        message, _, _ = decide({'messages': [{'role': 'user', 'content': request}]})
        assert message['content'] == expected
    if len(sys.argv) != 3:
        raise SystemExit('usage: send_now.py STAGED_PAIR EVIDENCE_DIRECTORY')
    stage = Path(sys.argv[1]).resolve()
    evidence = Path(sys.argv[2]).resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    hashes = {name: hashlib.sha256((stage / name).read_bytes()).hexdigest() for name in ['aster', 'a3s-code-acp']}
    with tempfile.TemporaryDirectory(prefix='aster-send-now-') as temporary:
        root = Path(temporary)
        workspace, home = root / 'workspace', root / 'home'
        workspace.mkdir()
        home.mkdir()
        (workspace / 'sentinel.txt').write_text('unchanged\n')
        server = ThreadingHTTPServer(('127.0.0.1', 0), StreamHandler)
        server.events = evidence / 'model.jsonl'
        server.stop = threading.Event()
        thread = threading.Thread(target=server.serve_forever)
        thread.start()
        config = root / 'host.acl'
        config.write_text('default_model = "fixture/stream-control"\nproviders "fixture" {\n api_key = "synthetic-fixture-key"\n base_url = "http://127.0.0.1:' + str(server.server_port) + '/v1"\n models "stream-control" {}\n}\n')
        suite = root / 'suite.acl'
        suite.write_text('suite "send-now" {\n version = 1\n scenario "cancel-and-send" {\n surface = "tui"\n timeout_ms = 60000\n wait "ready" { text = "❯" }\n snapshot "ready-state" {}\n terminal_paste "first" { text = "' + FIRST + '" }\n press "send-first" { key = "Enter" }\n wait "active-stream" { text = "' + READY + '" }\n terminal_paste "second" { text = "' + SECOND + '" }\n press "send-now" { key = "Control+o" }\n wait "second-completed" { text = "' + DONE + '" }\n wait "turn-settled" { regex = "Worked for |Turn completed\\." }\n snapshot "completed" {}\n terminal_recording "recording" { path = "terminal/send-now.vt" }\n }\n}\n')
        (evidence / 'suite.acl').write_text(suite.read_text())
        relay = root / 'acp-trace-proxy'
        relay.write_bytes((REPO / 'apps/aster/tests/tui_913_real_llm/acp_trace_proxy.py').read_bytes())
        relay.chmod(0o700)
        env = dict(os.environ, HOME=str(home), GROK_HOME=str(home / 'grok'), A3S_CONFIG=str(config), A3S_ACP_AGENT_BIN=str(relay), A3S_CODE_TUI_TEST_ACP_REAL=str(stage / 'a3s-code-acp'), A3S_CODE_TUI_TEST_ACP_TRACE=str(evidence / 'acp.jsonl'), A3S_DISABLE_CC_SWITCH='1', A3S_DISABLE_GROK_ACCOUNT='1', A3S_DISABLE_CURSOR_ACCOUNT='1', TERM='xterm-256color', TERM_PROGRAM='Apple_Terminal')
        env.pop('A3S_LH_CANDIDATE_MANIFEST', None)
        try:
            admitted = subprocess.run(['a3s-test', 'check', str(suite), '--json'], cwd=REPO, capture_output=True, text=True)
            (evidence / 'check.json').write_text(admitted.stdout)
            assert admitted.returncode == 0, admitted.stderr
            command = ['a3s-test', 'run', str(suite), '--tui-executable', str(stage / 'aster'), '--tui-arg=--no-plan', '--tui-working-directory', str(workspace), '--tui-columns', '120', '--tui-rows', '40', '--json']
            result = subprocess.run(command, cwd=REPO, env=env, capture_output=True, text=True)
            (evidence / 'a3s-test.json').write_text(result.stdout)
            (evidence / 'stderr.log').write_text(result.stderr)
        finally:
            server.stop.set()
            server.shutdown()
            server.server_close()
            thread.join()
        events = [json.loads(line) for line in server.events.read_text().splitlines()] if server.events.exists() else []
        trace_file = evidence / 'acp.jsonl'
        trace = [json.loads(line) for line in trace_file.read_text().splitlines()] if trace_file.exists() else []
        requests = [row for row in trace if row['kind'] == 'prompt-request']
        responses = [row for row in trace if row['kind'] == 'prompt-response']
        facts = {}
        for directory in [workspace, home]:
            for path in directory.rglob('*.jsonl'):
                facts[str(path.relative_to(root))] = path.read_text()
        (evidence / 'facts.json').write_text(json.dumps(facts, indent=2))
        records = [json.loads(line) for text in facts.values() for line in text.splitlines()]
        tool_facts = [f for f in records if f.get('type') in ['tool.prepared', 'tool.result']]
        narratives = [f for f in records if f.get('type') == 'turn.complete' and f['payload'].get('terminal') == 'narrative']
        model_turns = [f for f in records if f.get('type') == 'model.turn']
        completed_answers = [f for f in model_turns if f['payload'].get('text') == DONE]
        interrupted_answers = [f for f in model_turns if f['payload'].get('text') == '(Response interrupted by the user.)']
        matched = len(requests) == len(responses) == 2 and all(request['rpc_id'] == response['rpc_id'] and request['session_id'] == response['session_id'] and response['error_code'] is None for request, response in zip(requests, responses))
        report = {'engine_exit_code': result.returncode, 'binary_sha256': hashes, 'requests': requests, 'responses': responses, 'events': events, 'tool_facts': len(tool_facts), 'narrative_completions': len(narratives), 'completed_answers': len(completed_answers), 'interrupted_answers': len(interrupted_answers), 'sentinel_unchanged': (workspace / 'sentinel.txt').read_text() == 'unchanged\n'}
        report['passed'] = result.returncode == 0 and matched and requests[0]['session_id'] == requests[1]['session_id'] and [r['stop_reason'] for r in responses] == ['cancelled', 'end_turn'] and sum(e['kind'] == 'first' for e in events) == sum(e['kind'] == 'second' for e in events) == 1 and any(e['kind'] == 'first-transport-closed' for e in events) and not tool_facts and len(narratives) == 2 and len(model_turns) == 2 and len(completed_answers) == len(interrupted_answers) == 1 and report['sentinel_unchanged']
        (evidence / 'results.json').write_text(json.dumps(report, indent=2) + '\n')
        assert all(hashlib.sha256((stage / name).read_bytes()).hexdigest() == value for name, value in hashes.items())
        print(json.dumps({'passed': report['passed'], 'engine_exit_code': result.returncode, 'response_stop_reasons': [r['stop_reason'] for r in responses]}), flush=True)
        return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
