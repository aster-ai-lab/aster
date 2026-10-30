import asyncio, hashlib, json, os, re, time
from pathlib import Path

BASE = Path(__file__).resolve().parent
SPEC = json.loads((BASE / 'probe-spec.json').read_text())
ROOT = Path('/Users/roylin/code/aster/work/github-issues-20261030')
GUIDELINES = (ROOT / 'apps/studio/src-tauri/src/agent_plane/delivery_contract.md').read_text().strip()

async def run_case(case):
    context = json.loads((BASE / f'{case}-context.json').read_text())
    env = os.environ.copy()
    env.update({'A3S_HOME': str(BASE / f'state-{case}'), 'A3S_STATE_HOME': str(BASE / f'state-{case}'), 'A3S_CODE_LOG_FILE': '0'})
    output, tools, events = [], [], []
    start = time.monotonic()
    result = {'case': case, 'model': SPEC['model'], 'toolCalls': tools}
    with (BASE / f'{case}.stderr.log').open('w') as stderr:
        proc = await asyncio.create_subprocess_exec(
            SPEC['nativeBinary'], '--cwd', str(BASE / 'workspace'), '--config', str(BASE / 'config.acl'),
            '--model', SPEC['model'], '--reasoning-effort', SPEC['reasoningEffort'],
            stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE, stderr=stderr, env=env, limit=1048576)
        async def rpc(identifier, method, params, timeout):
            proc.stdin.write((json.dumps({'jsonrpc': '2.0', 'id': identifier, 'method': method, 'params': params}) + '\n').encode())
            await proc.stdin.drain()
            deadline = time.monotonic() + timeout
            while True:
                line = await asyncio.wait_for(proc.stdout.readline(), max(0.001, deadline - time.monotonic()))
                if not line: raise RuntimeError('ACP closed before its response')
                message = json.loads(line)
                if message.get('id') == identifier:
                    if 'error' in message: raise RuntimeError(json.dumps(message['error'], ensure_ascii=False))
                    return message['result']
                if message.get('method') == 'session/update':
                    update = message.get('params', {}).get('update', {})
                    kind = update.get('sessionUpdate')
                    if kind == 'agent_message_chunk':
                        content = update.get('content', {})
                        if content.get('type') == 'text': output.append(content['text'])
                    elif kind == 'tool_call': tools.append({'id': update.get('toolCallId'), 'title': update.get('title')})
                    if kind not in ['agent_thought_chunk', 'agent_message_chunk']: events.append({'sessionUpdate': kind})
                elif 'method' in message and 'id' in message:
                    raise RuntimeError('Unexpected client operation: ' + str(message['method']))
        try:
            await rpc(1, 'initialize', {'protocolVersion': 1, 'clientCapabilities': {}}, 20)
            opened = await rpc(2, 'session/new', {'cwd': str(BASE / 'workspace'), 'mcpServers': [], '_meta': {'a3sHost': {'role': 'You are Aster, a local desktop assistant.', 'guidelines': GUIDELINES, 'officeTools': True, 'outputLanguage': 'zh-CN'}}}, 40)
            current = opened['models']['currentModelId']
            if current != SPEC['model']: raise RuntimeError('Selected model differs from the fixed test model: ' + current)
            turn = await rpc(3, 'session/prompt', {'sessionId': opened['sessionId'], 'prompt': [{'type': 'text', 'text': SPEC['prompt']}], '_meta': {'a3sTurn': {'directives': context['directives'], 'research': context['research']}}}, SPEC['turnTimeoutSeconds'])
            result['stopReason'] = turn.get('stopReason')
        except Exception as error:
            result['error'] = type(error).__name__ + ': ' + str(error)
        finally:
            if proc.returncode is None:
                proc.terminate()
                try: await asyncio.wait_for(proc.wait(), 10)
                except asyncio.TimeoutError: proc.kill(); await proc.wait()
    text = ''.join(output)
    table_separators = re.findall(r'^\s*\|?\s*:?-{3,}:?\s*\|(?:\s*:?-{3,}:?\s*\|?)+\s*$', text, re.M)
    result.update({'durationSeconds': round(time.monotonic() - start, 3), 'tableCount': len(table_separators), 'endMarkerPresent': text.rstrip().endswith(SPEC['requiredEndMarker']), 'allHeadingsPresent': all(x in text for x in SPEC['requiredHeadings']), 'replyCharacters': len(text), 'replySha256': hashlib.sha256(text.encode()).hexdigest()})
    result['passed'] = 'error' not in result and not tools and result['tableCount'] == SPEC['requiredTables'] and result['endMarkerPresent'] and result['allHeadingsPresent']
    (BASE / f'{case}.reply.txt').write_text(text)
    (BASE / f'{case}.result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(result, ensure_ascii=False), flush=True)
    return result

async def main():
    results = []
    for case in ['before', 'after']: results.append(await run_case(case))
    (BASE / 'probe-results.json').write_text(json.dumps(results, ensure_ascii=False, indent=2) + '\n')

asyncio.run(main())
