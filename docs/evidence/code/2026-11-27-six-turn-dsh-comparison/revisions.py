#!/usr/bin/env python3
"""Run the exported native-file workflow through Aster and the independent DSH ACP."""

import argparse
import asyncio
import json
import os
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / '2026-11-27-model-selected-tool-catalog'))
from native import base, candidate_with_catalog

PROTECTED = ('README.md', 'notes.txt', 'order.json', 'policy.json')
REPORT = 'report.json'
MAX_BYTES = 128 * 1024
MAX_TOKENS = 4096
MAX_REQUESTS = 40


class RevisionAcp(base.Acp):
    def __init__(self, child, workspace):
        super().__init__(child, workspace)
        self.tool_updates = {}
        self.turn_outcomes = []

    def observe(self, packet):
        params = packet.get('params') or {}
        method = packet.get('method')
        if method == 'session/update':
            event = params['update']
            if event.get('sessionUpdate') in ('tool_call', 'tool_call_update'):
                self.tool_updates.setdefault(event['toolCallId'], {}).update(event)
        elif method in ('_aster/turn/outcome', 'aster/turn/outcome'):
            self.turn_outcomes.append(params.get('outcome', {}))

    def permit(self, info):
        call = info['toolCall']
        # DSH's ACP permission envelope refers to the earlier request by ID.
        call = {**self.tool_updates.get(call['toolCallId'], {}), **call}
        raw = call.get('rawInput')
        if not isinstance(raw, dict) or raw.get('sandbox_permissions') == 'require_escalated':
            return False
        location = raw.get('file_path') or raw.get('path')
        if not isinstance(location, str):
            return False
        root = self.workspace.resolve()
        target = (root / location).resolve()
        if call.get('kind') == 'read' or call.get('title') == 'read':
            return target in {root / filename for filename in (*PROTECTED, REPORT)}
        if call.get('kind') == 'edit' or call.get('title') in ('write', 'edit'):
            return target == root / REPORT
        return False


def artifact(workspace, filename):
    path = workspace / filename
    if path.is_symlink() or not path.is_file():
        return None
    with path.open('rb') as contents:
        data = contents.read(MAX_BYTES + 1)
    return data if len(data) <= MAX_BYTES else None


def observe_report(workspace, expected):
    raw = artifact(workspace, REPORT)
    try:
        report = json.loads(raw.decode('utf-8')) if raw is not None else None
    except (UnicodeError, ValueError):
        return None, False
    return report, (report == expected and isinstance(report, dict)
                    and all(type(report[key]) is type(value) for key, value in expected.items()))


async def run_trial(label, arm, binary, args, fixture, relay):
    out = args.out / label
    out.mkdir()
    workspace = args.out / 'workspace'
    home = args.out / 'home'
    workspace.mkdir()
    home.mkdir()
    protected = {'README.md': fixture['readme'].encode(), 'notes.txt': fixture['notes'].encode()}
    for filename, data in protected.items():
        (workspace / filename).write_bytes(data)
    command, env = base.acp_process(arm, binary, out, workspace, home, relay, args.embedding, max_tokens=MAX_TOKENS)
    relay.take(label, 0)
    with (out / 'stderr.log').open('wb') as errors:
        child = await asyncio.create_subprocess_exec(*command, cwd=workspace, env=env, stdin=asyncio.subprocess.PIPE,
                                                     stdout=asyncio.subprocess.PIPE, stderr=errors, limit=2**22)
    acp = RevisionAcp(child, workspace)
    observed = {'label': label, 'arm': arm, 'turns': []}
    try:
        response, *_ = await acp.call('initialize', {'protocolVersion': 1, 'clientCapabilities': {}}, 45)
        if response.get('result', {}).get('protocolVersion') != 1:
            raise RuntimeError('ACP initialize refused')
        request = {'cwd': str(workspace), 'mcpServers': []}
        if arm != 'dsh':
            request['_meta'] = {'asterHost': {'completionAttestation': 'disk_content'}}
        response, *_ = await acp.call('session/new', request, 45)
        session = response.get('result', {}).get('sessionId')
        if session is None:
            raise RuntimeError('ACP session refused')
        for index, turn in enumerate(fixture['turns'], 1):
            current = {**protected, 'order.json': turn['order'].encode(), 'policy.json': turn['policy'].encode()}
            for filename in ('order.json', 'policy.json'):
                (workspace / filename).write_bytes(current[filename])
            relay.take(label, index)
            acp.turn_outcomes.clear()
            answer, text, perms, tools = await acp.call('session/prompt', {
                'sessionId': session,
                'prompt': [{'type': 'text', 'text': turn['prompt']}],
            }, 600)
            report, correct = observe_report(workspace, turn['expected'])
            source_ok = all(artifact(workspace, name) == data for name, data in current.items())
            sealed = [o for o in acp.turn_outcomes if o.get('terminal') == 'verified'
                      and o.get('status') == 'passed' and o.get('requiredChecks', 0) > 0]
            status = answer.get('result', {}).get('stopReason')
            row = {'turn': index, 'stop': status, 'report': report, 'correct_report': correct,
                   'protected_files_exact': source_ok, 'bound_verification': bool(sealed) if arm != 'dsh' else None,
                   'permissions': perms, 'settled_tools': bool(tools) and all(t.get('status') in ('completed', 'failed') for t in tools)}
            observed['turns'].append(row)
            (out / f'answer-{index}.txt').write_text(text, encoding='utf-8')
            if 'error' in answer:
                (out / f'error-{index}.json').write_text(json.dumps(answer['error'], indent=2), encoding='utf-8')
            if status != 'end_turn' or not source_ok or not correct or (arm != 'dsh' and not sealed):
                break
    except (RuntimeError, OSError, asyncio.TimeoutError, ValueError) as error:
        observed['runner_error'] = str(error) if type(error) is RuntimeError else type(error).__name__
    finally:
        observed['meter_drained'] = await relay.settle_usage(label)
        await base.close_acp(child)
        workspace.rename(out / 'workspace')
        home.rename(out / 'home')
    rows = [row for row in relay.rows if row['trial'] == label]
    observed.update(base.summarise_usage(rows))
    names = set(observed['tool_calls'])
    observed['native_workflow'] = (bool(names & {'read', 'batch'}) and bool(names & {'write', 'edit', 'batch'})
                                   and names <= {'read', 'write', 'edit', 'batch', 'load_tool_definitions'})
    observed['passed'] = (len(observed['turns']) == len(fixture['turns']) and observed['native_workflow']
                          and all(t['stop'] == 'end_turn' and t['correct_report'] and t['protected_files_exact']
                                  and t['settled_tools'] and (arm == 'dsh' or t['bound_verification'])
                                  for t in observed['turns']))
    return observed


async def compare(args):
    args.out.mkdir()
    fixture = json.loads(args.fixture.read_text(encoding='utf-8'))
    assert len(fixture['turns']) == 6
    binary = {'candidate': candidate_with_catalog(args.out / 'candidate-launcher', args.acp), 'dsh': args.dsh,
              'release': candidate_with_catalog(args.out / 'reference-launcher', args.reference_acp)
              if args.reference_deferred else args.reference_acp}
    (args.out / 'manifest.json').write_text(json.dumps({
        'fixture': fixture, 'model': base.MODEL, 'order': args.order,
        'thinking': 'disabled', 'temperature': 0, 'max_tokens': MAX_TOKENS, 'max_requests': MAX_REQUESTS,
        'reference_catalog': 'deferred' if args.reference_deferred else 'full',
    }, indent=2) + '\n', encoding='utf-8')
    relay = base.Relay(os.environ.pop('DEEPSEEK_API_KEY'), args.out / 'provider-usage.jsonl',
                      max_tokens=MAX_TOKENS, max_requests=MAX_REQUESTS)
    results = []
    try:
        for position, arm in enumerate(args.order, 1):
            row = await run_trial(f'{position}-{arm}', arm, binary[arm], args, fixture, relay)
            results.append(row)
            (args.out / 'trials.json').write_text(json.dumps(results, indent=2) + '\n', encoding='utf-8')
            print(json.dumps(row), flush=True)
    finally:
        relay.server.shutdown()
        relay.server.server_close()


def main():
    parser = argparse.ArgumentParser()
    for name in ('fixture', 'acp', 'reference-acp', 'dsh', 'out', 'embedding'):
        parser.add_argument('--' + name, type=lambda value: Path(value).resolve(), required=name != 'embedding')
    parser.add_argument('--order', choices=['release', 'candidate', 'dsh'], nargs='+', required=True)
    parser.add_argument('--reference-deferred', action='store_true')
    asyncio.run(compare(parser.parse_args()))


if __name__ == '__main__':
    main()
