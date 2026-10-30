#!/usr/bin/env python3
"""Exercise real fullscreen terminal resizing and conversation with A3S Test."""
import hashlib
from http.server import ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import uuid

from json_render_model import Handler, decide as housekeeping


def main():
    if len(sys.argv) != 3:
        raise SystemExit('usage: render_resize.py STAGED_PAIR EVIDENCE_DIRECTORY')
    repo = Path(__file__).resolve().parents[2]
    stage = Path(sys.argv[1]).resolve()
    evidence = Path(sys.argv[2]).resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    hashes = {name: hashlib.sha256((stage / name).read_bytes()).hexdigest()
              for name in ['aster', 'a3s-code-acp']}
    replies = {name: name.upper() + '-' + uuid.uuid4().hex[:12]
               for name in ['narrow', 'wide']}

    class Model(Handler):
        model_name = 'render-fixture'

        @staticmethod
        def decide(body):
            messages = body.get('messages', [])
            prompt = next((m.get('content') for m in reversed(messages)
                           if m.get('role') == 'user'), None)
            if prompt in replies:
                return ({'role': 'assistant', 'content': replies[prompt]}, 'stop',
                        {'kind': 'render-reply', 'mode': prompt})
            return housekeeping(body)

    for mode, reply in replies.items():
        message, stop, event = Model.decide({'messages': [{'role': 'user', 'content': mode}]})
        assert message['content'] == reply and stop == 'stop' and event['mode'] == mode
    server = ThreadingHTTPServer(('127.0.0.1', 0), Model)
    server.events = evidence / 'model.jsonl'
    thread = threading.Thread(target=server.serve_forever)
    thread.start()
    try:
        with tempfile.TemporaryDirectory(prefix='aster-render-') as temporary:
            root = Path(temporary)
            home, workspace = root / 'home', root / 'workspace'
            home.mkdir()
            workspace.mkdir()
            config = root / 'host.acl'
            config.write_text('default_model = "fixture/render-fixture"\n'
                              'providers "fixture" {\n api_key = "synthetic-fixture-key"\n'
                              f' base_url = "http://127.0.0.1:{server.server_port}/v1"\n'
                              ' models "render-fixture" {}\n}\n')
            suite = root / 'suite.acl'
            suite.write_text('''suite "render-resize" {
 version = 1
 scenario "fullscreen-resize" {
  surface = "tui"
  timeout_ms = 60000
  wait "ready" { text = "❯" }
  terminal_resize "fullscreen-narrow" { columns = 60 rows = 20 }
  terminal_paste "fullscreen-request" { text = "narrow" }
  press "send-fullscreen" { key = "Enter" }
  wait "fullscreen-reply" { text = "''' + replies['narrow'] + '''" }
  wait "fullscreen-settled" { regex = "Worked for |Turn completed\\." }
  snapshot "fullscreen-narrow-frame" {}
  terminal_resize "fullscreen-wide" { columns = 100 rows = 30 }
  terminal_paste "wide-request" { text = "wide" }
  press "send-wide" { key = "Enter" }
  wait "wide-reply" { text = "''' + replies['wide'] + '''" }
  snapshot "fullscreen-wide-frame" {}
  terminal_resize "fullscreen-restored" { columns = 120 rows = 40 }
  snapshot "restored-frame" {}
  terminal_recording "recording" { path = "terminal/render-resize.vt" }
 }
}
''')
            (evidence / 'suite.acl').write_text(suite.read_text())
            relay = root / 'acp-trace-proxy'
            relay.write_bytes((repo / 'apps/aster/tests/tui_913_real_llm/acp_trace_proxy.py').read_bytes())
            relay.chmod(0o700)
            env = dict(os.environ, HOME=str(home), GROK_HOME=str(home / 'grok'),
                       A3S_CONFIG=str(config), A3S_ACP_AGENT_BIN=str(relay),
                       A3S_CODE_TUI_TEST_ACP_REAL=str(stage / 'a3s-code-acp'),
                       A3S_CODE_TUI_TEST_ACP_TRACE=str(evidence / 'acp.jsonl'),
                       A3S_DISABLE_CC_SWITCH='1', A3S_DISABLE_GROK_ACCOUNT='1',
                       A3S_DISABLE_CURSOR_ACCOUNT='1', TERM='xterm-256color')
            env.pop('A3S_LH_CANDIDATE_MANIFEST', None)
            checked = subprocess.run(['a3s-test', 'check', str(suite), '--json'],
                                     cwd=repo, capture_output=True, text=True)
            (evidence / 'check.json').write_text(checked.stdout)
            assert checked.returncode == 0, checked.stderr
            command = ['a3s-test', 'run', str(suite), '--tui-executable', str(stage / 'aster'),
                       '--tui-arg=--no-plan', '--tui-working-directory', str(workspace),
                       '--tui-columns', '120', '--tui-rows', '40', '--json']
            result = subprocess.run(command, cwd=repo, env=env, capture_output=True, text=True)
            (evidence / 'a3s-test.json').write_text(result.stdout)
            (evidence / 'stderr.log').write_text(result.stderr)
            report = json.loads((evidence / 'a3s-test.json').read_text())
            scenario = report['scenarios'][0]
            frames = {step['id']: step['output']['data']['viewport']
                      for step in scenario['steps']
                      if step.get('id') in ['fullscreen-narrow-frame', 'fullscreen-wide-frame', 'restored-frame']}
            sizes = {name: [frame['columns'], frame['rows']] for name, frame in frames.items()}
            events = ([json.loads(line) for line in server.events.read_text().splitlines()]
                      if server.events.exists() else [])
            modes = [event['mode'] for event in events if event.get('kind') == 'render-reply']
            receipt = {'exit_code': result.returncode, 'status': report['status'],
                       'sizes': sizes, 'model_turns': modes, 'binary_sha256': hashes,
                       'cleanup_error': scenario['cleanup_error'], 'test_source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
            (evidence / 'results.json').write_text(json.dumps(receipt, indent=2) + '\n')
            print(json.dumps(receipt), flush=True)
            assert result.returncode == 0 and report['status'] == 'passed'
            assert scenario['cleanup_error'] is None
            assert sizes == {'fullscreen-narrow-frame': [60, 20], 'fullscreen-wide-frame': [100, 30], 'restored-frame': [120, 40]}
            assert modes == ['narrow', 'wide']
            assert frames['fullscreen-narrow-frame']['alternate_screen'] is True
            assert frames['fullscreen-wide-frame']['alternate_screen'] is True
            assert frames['restored-frame']['alternate_screen'] is True
    finally:
        server.shutdown()
        server.server_close()
        thread.join()
    assert all(hashlib.sha256((stage / name).read_bytes()).hexdigest() == digest
               for name, digest in hashes.items())


if __name__ == '__main__':
    main()
