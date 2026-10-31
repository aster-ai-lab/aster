#!/usr/bin/env python3
"""Run actual TUI tools through Aster Test with synthetic model requests."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
from http.server import ThreadingHTTPServer

REPO = Path(__file__).resolve().parents[2]
WORK = REPO / 'target/tui-capability-baseline'
sys.path.insert(0, str(REPO / 'tests/e2e'))
from json_render_model import Handler, decide as housekeeping

TOKEN = 'CAPABILITY-READ-6a8c'
SKILL_TOKEN = 'CAPABILITY-SKILL-53e1'
CASES = {
    'symbols': ('code_symbols', {'operation': 'outline', 'path': 'src/probe.ts'}, 'capabilityProbe'),
    'symbols-unavailable': ('code_symbols', {'operation': 'outline', 'path': 'src/probe.ts'}, 'Could not find a valid TypeScript installation'),
    'program-read': ('program', {'type': 'script', 'allowed_tools': ['read'], 'source': 'async function run(ctx, inputs) { return await ctx.tool("read", {file_path:"note.txt"}); }'}, TOKEN),
    'skill': ('Skill', {'skill_name': 'capability-probe'}, SKILL_TOKEN),
    'program-denied': ('program', {'type': 'script', 'allowed_tools': ['read'], 'source': 'async function run(ctx, inputs) { return await ctx.tool("write", {file_path:"sentinel.txt", content:"changed"}); }'}, "tool 'write' is not allowed for this PTC script"),
    'program-missing-use-runtime': ('program', {'type': 'script', 'allowed_tools': ['use_runtime_task'], 'source': 'async function run(ctx, inputs) { return await ctx.tool("use_runtime_task", {task:"probe"}); }'}, 'Unknown tool: use_runtime_task'),
    'mcp': ('mcp__fixture__lookup', {}, 'fixture'),
    'plan': ('update_plan', {'plan': [{'step': 'Inspect source', 'status': 'completed'}, {'step': 'Verify result', 'status': 'in_progress'}]}, 'Updated plan: 1/2 done · in progress: Verify result'),
    'git-status': ('git', {'command': 'status'}, 'uncommitted change(s)'),
    'search-grep': ('search', {'mode': 'grep', 'query': TOKEN, 'path': 'note.txt'}, TOKEN),
    'search-glob': ('search', {'mode': 'glob', 'query': '**/probe.ts'}, 'src/probe.ts'),
    'list-directory': ('ls', {'path': 'src'}, 'probe.ts'),
    'headless-refusal': ('web_search', {'query': 'kernel bound', 'engines': ['google']}, 'No results found for query:'),
    'patch-file': ('patch', {'file_path': 'patch-target.txt', 'diff': '@@ -1 +1 @@\n-original\n+patched\n'}, 'Applied 1 hunk(s)'),
    'patch-atomic-refusal': ('patch', {'patches': [{'file_path': 'patch-target.txt', 'diff': '@@ -1 +1 @@\n-original\n+patched\n'}, {'file_path': 'sentinel.txt', 'diff': '@@ -1 +1 @@\n-wrong preimage\n+changed\n'}]}, 'mismatch'),
    'download-file': ('download', {'url': 'https://example.com/', 'file_path': 'downloaded.html', 'max_bytes': 65536, 'timeout': 20, 'connections': 1}, 'Downloaded '),
}
if sys.platform == 'darwin':
    CASES['background-refusal'] = ('background', {'action': 'start', 'name': 'baseline-probe', 'command': 'printf unexpected > background-refusal-target.txt'}, 'this sandbox backend does not support background execution')
REFUSED_CASES = {'symbols-unavailable', 'program-denied', 'headless-refusal', 'patch-atomic-refusal', 'background-refusal'}

def decide(body):
    raw = json.dumps(body)
    if 'emit_pre_analysis' in raw or 'You are a pre-analysis assistant' in raw or 'You extract durable, reusable memory' in raw:
        return housekeeping(body)
    messages = body.get('messages', [])
    user = next((m.get('content', '') for m in reversed(messages) if m.get('role') == 'user'), '')
    selected = [name for name in CASES if user == 'CAPABILITY-PROBE:' + name]
    if len(selected) != 1:
        raise ValueError('exact fixture case required')
    case = selected[0]
    tool, args, expected = CASES[case]
    call_id = 'capability-' + case
    calls = [call for message in messages if message.get('role') == 'assistant' for call in message.get('tool_calls', [])]
    if calls:
        assert len(calls) == 1 and calls[0]['function']['name'] == tool
        assert json.loads(calls[0]['function']['arguments']) == args
    receipts = [m.get('content', '') for m in messages if m.get('role') == 'tool' and calls and m.get('tool_call_id') == calls[0]['id']]
    event = {'case': case, 'tool': tool}
    if receipts:
        content = str(receipts[-1])
        status, separator, output = content.partition('\n')
        expected_status = 'Tool exit code: ' + ('1' if case in REFUSED_CASES else '0')
        accepted = bool(separator) and status == expected_status and expected in output
        if case == 'background-refusal':
            accepted = content == expected
        if case == 'program-missing-use-runtime':
            try:
                nested = json.loads(output.partition('\nResult:\n')[2])
            except ValueError:
                nested = {}
            accepted = accepted and nested.get('name') == 'use_runtime_task' and nested.get('exitCode') == 1 and nested.get('output') == expected
        event.update(kind='tool_result', accepted=accepted, output=content[:2400])
        return {'role': 'assistant', 'content': 'CAPABILITY-RESULT:' + case + (':PASS' if accepted else ':FAIL')}, 'stop', event
    offered = any(t.get('function', {}).get('name') == tool for t in body.get('tools', []))
    event.update(kind='tool_call', offered=offered)
    if not offered:
        event['offered_tools'] = [t.get('function', {}).get('name') for t in body.get('tools', [])]
    if not offered:
        return {'role': 'assistant', 'content': 'CAPABILITY-MISSING:' + case}, 'stop', event
    call = {'id': call_id, 'type': 'function', 'function': {'name': tool, 'arguments': json.dumps(args)}}
    return {'role': 'assistant', 'content': None, 'tool_calls': [call]}, 'tool_calls', event

class ModelHandler(Handler):
    model_name = 'capability-fixture'
    decide = staticmethod(decide)

def self_check():
    for name, (tool, args, expected) in CASES.items():
        body = {'messages': [{'role': 'user', 'content': 'CAPABILITY-PROBE:' + name}], 'tools': [{'function': {'name': tool}}]}
        first, finish, event = decide(body)
        assert finish == 'tool_calls' and event['offered']
        assert json.loads(first['tool_calls'][0]['function']['arguments']) == args
        status = 'Tool exit code: ' + ('1' if name in REFUSED_CASES else '0')
        receipt = expected
        if name == 'program-missing-use-runtime':
            receipt = 'Program script completed.\nResult:\n' + json.dumps({'name':'use_runtime_task','exitCode':1,'output':expected})
        valid_content = receipt if name == 'background-refusal' else status + '\n' + receipt
        for content, passed in [('unrelated receipt', False), (valid_content, True)]:
            remapped = {**first['tool_calls'][0], 'id':'runtime-owned-identity'}
            second = {**body, 'messages': body['messages'] + [{'role':'assistant', 'tool_calls':[remapped]}, {'role':'tool', 'tool_call_id':remapped['id'], 'content':content}]}
            message, finish, event = decide(second)
            assert event['accepted'] == passed and finish == 'stop'
            assert message['content'].endswith(':PASS' if passed else ':FAIL')
        missing, _, event = decide({'messages':body['messages']})
        assert not event['offered'] and ':PASS' not in missing['content']

def main():
    self_check()
    if len(sys.argv) != 3:
        raise SystemExit('usage: capability_baseline.py STAGED_PAIR EVIDENCE_DIRECTORY')
    WORK.mkdir(parents=True, exist_ok=True)
    stage = Path(sys.argv[1]).resolve()
    results = []
    binary_hashes = {name: hashlib.sha256((stage / name).read_bytes()).hexdigest() for name in ['aster', 'aster-acp']}
    evidence = Path(sys.argv[2]).resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    for case in CASES:
        with tempfile.TemporaryDirectory(prefix='capability-fixture-', dir=WORK) as temporary:
            root = Path(temporary)
            workspace = root / 'workspace'
            home = root / 'home'
            workspace.mkdir()
            home.mkdir()
            (workspace / 'src').mkdir()
            (workspace / 'package.json').write_text('{"name":"capability-fixture","private":true}')
            (workspace / 'tsconfig.json').write_text('{"compilerOptions":{"noEmit":true},"include":["src"]}')
            if case == 'symbols':
                typescript = REPO / 'packages/intelligent-ui/node_modules/typescript'
                assert (typescript / 'lib/tsserver.js').is_file(), 'positive project requires the existing repository TypeScript dependency'
                (workspace / 'node_modules').mkdir()
                (workspace / 'node_modules/typescript').symlink_to(typescript, target_is_directory=True)
            (workspace / 'src/probe.ts').write_text('export function capabilityProbe(value: number): number { return value + 1; }\n')
            (workspace / 'note.txt').write_text(TOKEN + '\n')
            (workspace / 'sentinel.txt').write_text('unchanged\n')
            (workspace / 'patch-target.txt').write_text('original\n')
            if case == 'git-status':
                templates = root / 'empty-git-templates'
                templates.mkdir()
                git_env = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM='1', GIT_TEMPLATE_DIR=str(templates))
                subprocess.run(['git', '-c', 'init.defaultBranch=main', 'init', '--quiet', str(workspace)], env=git_env, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            skills = workspace / '.agents/skills/capability-probe'
            skills.mkdir(parents=True)
            (skills / 'SKILL.md').write_text('---\nname: capability-probe\ndescription: Read the fixture skill body.\nkind: instruction\n---\n' + SKILL_TOKEN + '\n')
            events = evidence / (case + '-model.jsonl')
            assert not events.exists(), 'preserve prior test evidence'
            server = ThreadingHTTPServer(('127.0.0.1', 0), ModelHandler)
            server.events = events
            thread = threading.Thread(target=server.serve_forever)
            thread.start()
            try:
                config = root / 'host.acl'
                config.write_text('default_model = "fixture/capability-fixture"\nproviders "fixture" {\n api_key = "synthetic-fixture-key"\n base_url = "http://127.0.0.1:' + str(server.server_port) + '/v1"\n models "capability-fixture" {}\n}\n')
                if case == 'mcp':
                    with config.open('a') as output:
                        output.write('mcp_servers "fixture" {\n transport = "stdio"\n command = ' + json.dumps(sys.executable) + '\n args = [' + json.dumps(str(REPO / 'tests/e2e/capability_mcp.py')) + ']\n env = { ASTER_TEST_MCP_LOG = ' + json.dumps(str(evidence / (case + '-mcp-process.jsonl'))) + ' }\n}\n')
                suite = root / 'suite.acl'
                approval = (' wait "permission" { text = "Yes, proceed" }\n press "allow-once" { key = "1" }\n' if CASES[case][0] in ['program', 'Skill', 'mcp__fixture__lookup', 'patch', 'download'] else '')
                if case == 'program-missing-use-runtime':
                    approval += ' wait "nested-permission" { text = "Allow use_runtime_task?" }\n press "allow-nested-once" { key = "1" }\n'
                suite.write_text('suite "capability-baseline" {\n version = 1\n scenario "' + case + '" {\n surface = "tui"\n timeout_ms = 60000\n wait "ready" { text = "fixture/capability-fixture" }\n terminal_paste "request" { text = "CAPABILITY-PROBE:' + case + '" }\n press "send" { key = "Enter" }\n' + approval + ' wait "actual-tool-receipt" { text = "CAPABILITY-RESULT:' + case + ':PASS" }\n snapshot "completed" {}\n terminal_recording "receipt" { path = "terminal/capability.vt" }\n }\n}\n')
                relay = root / 'acp-trace-proxy'
                relay.write_bytes((REPO / 'apps/aster/tests/tui_913_real_llm/acp_trace_proxy.py').read_bytes())
                relay.chmod(0o700)
                env = dict(os.environ, HOME=str(home), GROK_HOME=str(home / 'grok'), ASTER_CONFIG=str(config), ASTER_ACP_AGENT_BIN=str(relay), ASTER_TUI_TEST_ACP_REAL=str(stage / 'aster-acp'), ASTER_TUI_TEST_ACP_TRACE=str(evidence / (case + '-acp.jsonl')), ASTER_DISABLE_CC_SWITCH='1', ASTER_DISABLE_GROK_ACCOUNT='1', ASTER_DISABLE_CURSOR_ACCOUNT='1', TERM='xterm-256color')
                env['ASTER_TEST_MCP_LOG'] = str(evidence / (case + '-mcp-process.jsonl'))
                env.pop('ASTER_LH_CANDIDATE_MANIFEST', None)
                command = ['aster-test', 'run', str(suite), '--tui-executable', str(stage / 'aster'), '--tui-arg=--no-plan', '--tui-working-directory', str(workspace), '--tui-columns', '120', '--tui-rows', '40', '--json']
                result = subprocess.run(command, cwd=REPO, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
                (evidence / (case + '-aster-test.json')).write_text(result.stdout)
                (evidence / (case + '-stderr.log')).write_text(result.stderr)
                (evidence / (case + '-suite.acl')).write_text(suite.read_text())
                records = [json.loads(line) for line in events.read_text().splitlines()] if events.exists() else []
                receipts = [r for r in records if r.get('kind') == 'tool_result']
                sentinel = (workspace / 'sentinel.txt').read_text() == 'unchanged\n'
                # Keep the authoritative run facts, independent of the fixture's final marker.
                fact_sources = {}
                for base in [workspace, home]:
                    for path in base.rglob('*.jsonl'):
                        fact_sources[str(path.relative_to(root))] = path.read_text()
                (evidence / (case + '-facts.json')).write_text(json.dumps(fact_sources, indent=2))
                row = {'case':case, 'engine_exit_code':result.returncode, 'tool_receipts':receipts, 'sentinel_unchanged':sentinel, 'binary_sha256':binary_hashes, 'passed':result.returncode == 0 and len(receipts) == 1 and receipts[0]['accepted'] and sentinel}
                row['background_target_absent'] = not (workspace / 'background-refusal-target.txt').exists()
                row['passed'] = row['passed'] and row['background_target_absent']
                if case == 'background-refusal':
                    denied = []
                    for text in fact_sources.values():
                        for line in text.splitlines():
                            fact = json.loads(line)
                            if fact.get('type') == 'tool.result':
                                denied.append(fact['payload']['receipt'])
                    row['background_refusal_receipts'] = denied
                    row['passed'] = row['passed'] and len(denied) == 1 and denied[0]['outcome'] == 'denied' and denied[0]['exit_code'] is None and denied[0]['output'] == CASES[case][2] and denied[0]['effects']['completeness'] == 'complete' and denied[0]['effects']['changes'] == []
                if case in ['git-status', 'headless-refusal', 'download-file']:
                    tool_results = []
                    for text in fact_sources.values():
                        for line in text.splitlines():
                            fact = json.loads(line)
                            if fact.get('type') == 'tool.result':
                                tool_results.append(json.loads(fact['payload']['receipt']['output']))
                    row['formal_tool_result_count'] = len(tool_results)
                    row['passed'] = row['passed'] and len(tool_results) == 1
                if case == 'git-status' and len(tool_results) == 1:
                    actual = subprocess.check_output(['git', '-C', str(workspace), 'status', '--porcelain', '--short'], env=git_env, text=True)
                    status = tool_results[0]
                    row['git_status'] = {'actual_porcelain':actual, 'reported':status['metadata']}
                    row['passed'] = row['passed'] and status['metadata']['dirty_count'] == len(actual.splitlines()) and status['metadata']['is_dirty'] is True and str(workspace) in status['text']
                if case == 'headless-refusal' and len(tool_results) == 1:
                    status = tool_results[0]
                    row['headless_refusal'] = status['metadata']
                    failures = status['metadata']['engine_failures']
                    row['passed'] = row['passed'] and status['exit_code'] == 1 and len(failures) == 1 and failures[0]['kind'] == 'headless_unavailable' and status['metadata']['selected_engines'] == ['google']
                expected_patch = 'patched\n' if case == 'patch-file' else 'original\n'
                row['patch_target'] = (workspace / 'patch-target.txt').read_text()
                row['passed'] = row['passed'] and row['patch_target'] == expected_patch
                if case == 'download-file' and len(tool_results) == 1:
                    downloaded = workspace / 'downloaded.html'
                    payload = downloaded.read_bytes() if downloaded.is_file() else b''
                    metadata = tool_results[0]['metadata']
                    row['download'] = {'bytes':len(payload), 'sha256':hashlib.sha256(payload).hexdigest(), 'reported':metadata}
                    row['external_dependency'] = 'https://example.com/; deterministic model, actual public HTTP transfer'
                    row['passed'] = row['passed'] and 0 < len(payload) <= 65536 and metadata['bytes'] == len(payload) and metadata['file_path'] == 'downloaded.html' and metadata['overwritten'] is False and 'https://example.com/' in metadata['source_anchors']
                results.append(row)
                (evidence / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
                print(json.dumps({'case':case, 'passed':row['passed'], 'engine_exit_code':result.returncode}), flush=True)
            finally:
                server.shutdown()
                server.server_close()
                thread.join()
    assert all(hashlib.sha256((stage / name).read_bytes()).hexdigest() == sha for name, sha in binary_hashes.items())
    return 0 if all(r['passed'] for r in results) else 1

if __name__ == '__main__':
    raise SystemExit(main())
