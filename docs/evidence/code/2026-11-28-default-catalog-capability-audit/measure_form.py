#!/usr/bin/env python3
"""Compare native form generation with and without the deferred ACP catalog."""

import argparse
import asyncio
import json
import os
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / '2026-11-27-model-selected-tool-catalog'))
from native import base, candidate_with_catalog

PROMPT = (
    '生成一个中文发运表单，请直接调用 ask_user_form 恰好一次。表单标题为“发运确认”。'
    '字段 region 为必填单选，提供“北线”north 和“南线”south 两项。'
    'quantity 是必填的数值输入，最小 1、最大 9、步长 1；enabled 是布尔复选框。'
    '显示含 North、South 两条的柱状图，值分别为 2 和 5。'
    '提交后只输出返回的三个字段的 JSON，不执行其他工作。'
)
VALUES = {'region': 'south', 'quantity': 7, 'enabled': True}
CONTRACT_PROMPT = (
    '请调用 ask_user_form 生成中文表单“发运确认”。region 是必填单选，选项依次为“北线”和“南线”。'
    'quantity 是必填数值输入，最小 1、最大 9、步长 1。enabled 是布尔复选框。'
    '显示含 North、South 两条的柱状图，值分别为 2 和 5。'
    '收到提交后只输出返回的三个字段的 JSON，不执行其他工作。'
)
CONTRACT_VALUES = {'region': '南线', 'quantity': 7, 'enabled': True}


class FormAcp(base.Acp):
    def __init__(self, child, workspace, values):
        super().__init__(child, workspace)
        self.forms = {}
        self.values = values

    def permit(self, info):
        return False

    def observe(self, packet):
        if packet.get('method') in ('aster/form/request', '_aster/form/request'):
            self.forms[packet['id']] = packet['params']

    async def send(self, packet):
        if 'error' in packet and packet.get('id') in self.forms:
            packet = {'id': packet['id'], 'result': {'outcome': 'submit', 'values': self.values}}
        await super().send(packet)


def inspect_form(request, valid_contract):
    document = json.loads(request['spec'])
    elements = document['elements']
    assert document['root'] in elements
    assert request['format'] == 'json-render'
    controls = {node['type']: node['props'] for node in elements.values() if node['type'] in {
        'Select', 'Input', 'Checkbox', 'BarChart', 'Button',
    }}
    assert controls['Select']['options'] == (
        ['北线', '南线'] if valid_contract else [
            {'label': '北线', 'value': 'north'}, {'label': '南线', 'value': 'south'},
        ]
    )
    assert controls['Select']['value'] == {'$bindState': '/form/region'}
    assert controls['Select']['required'] is True
    assert controls['Input']['value'] == {'$bindState': '/form/quantity'}
    assert controls['Input']['type'] == 'number' and controls['Input']['required'] is True
    assert [controls['Input'][k] for k in ('min', 'max', 'step')] == [1, 9, 1]
    assert controls['Checkbox']['checked'] == {'$bindState': '/form/enabled'}
    assert controls['BarChart']['labels'] == ['North', 'South']
    assert controls['BarChart']['values'] == [2, 5]
    assert controls['Button']['action'] == 'submit'
    assert '发运确认' in json.dumps(document, ensure_ascii=False)
    return document


async def run_trial(label, binary, args, relay):
    out = args.out / label
    out.mkdir()
    workspace, home = out / 'workspace', out / 'home'
    workspace.mkdir()
    home.mkdir()
    command, env = base.acp_process('candidate', binary, out, workspace, home, relay, args.embedding)
    relay.take(label, 1)
    with (out / 'stderr.log').open('wb') as errors:
        child = await asyncio.create_subprocess_exec(*command, cwd=workspace, env=env,
                                                     stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE,
                                                     stderr=errors, limit=2**22)
    valid_contract = args.fixture == 'contract'
    prompt, values = (CONTRACT_PROMPT, CONTRACT_VALUES) if valid_contract else (PROMPT, VALUES)
    client = FormAcp(child, workspace, values)
    try:
        initialized, *_ = await client.call('initialize', {'protocolVersion': 1, 'clientCapabilities': {}})
        assert initialized['result']['protocolVersion'] == 1
        opened, *_ = await client.call('session/new', {'cwd': str(workspace), 'mcpServers': []})
        session = opened['result']['sessionId']
        terminal, answer, permissions, _ = await client.call('session/prompt', {
            'sessionId': session, 'prompt': [{'type': 'text', 'text': prompt}],
        }, timeout=180)
    finally:
        await base.close_acp(child)
    assert await relay.settle_usage(label)
    result = base.summarise_usage([row for row in relay.rows if row['trial'] == label])
    forms = list(client.forms.values())
    try:
        (form,) = forms
        inspect_form(form, valid_contract)
        shape_ok = True
    except (AssertionError, KeyError, ValueError, TypeError):
        shape_ok = False
    try:
        result_values = json.loads(answer.strip().removeprefix('```json').removesuffix('```').strip())
    except ValueError:
        result_values = None
    tools = result['tool_calls'].copy()
    tools.pop('load_tool_definitions', None)
    fresh_files = sorted(str(p.relative_to(workspace)) for p in workspace.iterdir() if p.name != '.aster')
    expected_tools = tools.keys() == {'ask_user_form'} if valid_contract else tools == {'ask_user_form': 1}
    return {'label': label, 'forms': forms, 'shape_ok': shape_ok, 'response': answer,
            'permissions': permissions, 'passed': shape_ok and result_values == values and not fresh_files
            and expected_tools and terminal.get('result', {}).get('stopReason') == 'end_turn'
            and result['usage_complete'], **result}


async def measure(args):
    args.out.mkdir()
    launcher = candidate_with_catalog(args.out / 'launcher', args.acp)
    sizes = {'default': args.acp, 'deferred': launcher}
    relay = base.Relay(os.environ.pop('DEEPSEEK_API_KEY'), args.out / 'provider-usage.jsonl', max_requests=12)
    rows = []
    try:
        for index, order in enumerate((('default', 'deferred'), ('deferred', 'default')), 1):
            for arm in order:
                row = await run_trial(f'{index}-{arm}', sizes[arm], args, relay)
                rows.append({'arm': arm, **row})
                print(row['label'], row['passed'], row['tool_calls'], row['tokens'], flush=True)
                (args.out / 'trials.json').write_text(json.dumps(rows, indent=2, ensure_ascii=False) + '\n')
    finally:
        relay.server.shutdown()
        relay.server.server_close()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ('acp', 'embedding', 'out'):
        parser.add_argument('--' + key, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--fixture', choices=('prototype', 'contract'), default='prototype')
    asyncio.run(measure(parser.parse_args()))
