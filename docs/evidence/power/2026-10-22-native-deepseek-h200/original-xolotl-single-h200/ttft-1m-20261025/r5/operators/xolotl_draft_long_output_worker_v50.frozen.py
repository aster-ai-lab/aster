"""Actual native tool roundtrip and matched1024-token single-request output, public data only."""
import hashlib
import json
import os
import socket
import statistics
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PLAN = json.loads((ROOT / 'request-plan.json').read_text())
SERVICE = Path(PLAN['service_root'])
STATE = dict(schema='aster-xolotl-draft-long-output-v50', unix=time.time(), phase='preflight',
             profile=PLAN['profile'], client=socket.gethostname(), tests=[],
             scope='Single active API request; ctx1M configured, short public prompts,1024 generated tokens; not actual1M input or cold concurrency.',
             full1m_input_qualified=False, speed_300_qualified=False)
OPENER = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def save():
    raw = (json.dumps(STATE, indent=2) + '\n').encode()
    if len(raw) > 1024**2:
        raise RuntimeError('public long-output receipt byte bound exceeded')
    temporary = ROOT / 'request-result.tmp'
    temporary.write_bytes(raw)
    temporary.replace(ROOT / 'request-result.json')


def ask(name, body):
    key = (SERVICE / 'credentials/api-key.txt').read_text().strip()
    payload = dict(model='deepseek-v4.1-flash-1017-xq', temperature=0, seed=1, reasoning_effort='none',
                   stream=True, stream_options=dict(include_usage=True), **body)
    raw = json.dumps(payload).encode()
    if len(raw) > 65536:
        raise RuntimeError('short public long-output request exceeded payload bound')
    request = urllib.request.Request(PLAN['base_url'] + '/chat/completions', data=raw,
        headers={'Authorization': 'Bearer ' + key, 'Content-Type': 'application/json'})
    start = time.monotonic()
    first = last = None
    content = ''
    usage = finish = None
    calls = {}
    done = False
    received = 0
    STATE['current_request'] = name
    save()
    try:
        response = OPENER.open(request, timeout=180)
    except urllib.error.HTTPError as error:
        text = error.read(4096).decode(errors='replace').replace(key, '[REDACTED]')
        raise RuntimeError(name + ' HTTP' + str(error.code) + ': ' + text) from error
    with response:
        status = response.status
        for line in response:
            received += len(line)
            if received > 2 * 1024**2 or time.monotonic() - start > 180:
                raise RuntimeError('bounded long-output response/time exceeded')
            if not line.startswith(b'data:'):
                continue
            value = line[5:].strip()
            if value == b'[DONE]':
                done = True
                break
            item = json.loads(value)
            if item.get('error'):
                raise RuntimeError(name + ' SSE error: ' + json.dumps(item['error']).replace(key, '[REDACTED]'))
            for choice in item.get('choices', []):
                delta = choice.get('delta', {})
                part = delta.get('content') or ''
                if part:
                    last = time.monotonic()
                    first = first or last
                    content += part
                for call in delta.get('tool_calls') or []:
                    index = call.get('index', 0)
                    if type(index) is not int or index != 0:
                        raise RuntimeError('unexpected native tool call index')
                    target = calls.setdefault(index, dict(id='', type='function', function=dict(name='', arguments='')))
                    if call.get('id'):
                        target['id'] = call['id']
                    function = call.get('function') or {}
                    target['function']['name'] += function.get('name') or ''
                    target['function']['arguments'] += function.get('arguments') or ''
                finish = choice.get('finish_reason') or finish
            usage = item.get('usage') or usage
            if len(content) > 65536 or len(json.dumps(calls)) > 8192:
                raise RuntimeError('native completion/tool output byte bound exceeded')
    row = dict(name=name, status=status, done=done, finish_reason=finish, content=content,
               output_sha256=hashlib.sha256(content.encode()).hexdigest(), tool_calls=list(calls.values()), usage=usage,
               seconds=time.monotonic() - start, ttft_seconds=first - start if first is not None else None,
               first_to_last_text_seconds=last - first if first is not None else None)
    STATE['tests'].append(row)
    save()
    if status != 200 or not done or not finish or usage is None:
        raise RuntimeError(name + ' did not finish completely')
    return row


try:
    os.umask(0o077)
    save()
    key = (SERVICE / 'credentials/api-key.txt').read_text().strip()
    request = urllib.request.Request(PLAN['base_url'].removesuffix('/v1') + '/health',
                                    headers={'Authorization': 'Bearer ' + key})
    with OPENER.open(request, timeout=8) as response:
        health = json.load(response)
    if not health.get('loaded') or health.get('max_context') != 1048576:
        raise RuntimeError('actual isolated1M API must be ready')
    tools = [dict(type='function', function=dict(name='add_numbers', description='Add two integers.',
        parameters=dict(type='object', properties=dict(a=dict(type='integer'), b=dict(type='integer')), required=['a', 'b'])))]
    instruction = 'Call add_numbers with a=17 and b=25. When the tool returns, reply with only the integer answer.'
    STATE['phase'] = 'native_tool_roundtrip'
    save()
    tool = ask('forced-native-tool-stream', dict(messages=[dict(role='user', content=instruction)], max_tokens=128,
        tools=tools, tool_choice=dict(type='function', function=dict(name='add_numbers'))))
    if tool['finish_reason'] != 'tool_calls' or len(tool['tool_calls']) != 1:
        raise RuntimeError('native streamed tool call did not finish as a single tool call')
    call = tool['tool_calls'][0]
    if not call['id'] or call['function']['name'] != 'add_numbers' or json.loads(call['function']['arguments']) != dict(a=17, b=25):
        raise RuntimeError('complete native tool arguments were not preserved')
    answer = ask('native-tool-followup-42', dict(messages=[dict(role='user', content=instruction),
        dict(role='assistant', content=tool['content'] or '', tool_calls=[call]),
        dict(role='tool', tool_call_id=call['id'], content='42')], tools=tools, max_tokens=128))
    if answer['content'].strip() != '42' or answer['tool_calls']:
        raise RuntimeError('actual native tool-result followup failed')
    STATE['native_tool_roundtrip_passed'] = True
    save()
    cases = [
        dict(name='rust-lru-1024', prompt='Write a comprehensive Rust tutorial of at least 2500 words implementing an LRU cache from scratch. Include the full generic data structure, all methods, error handling, unit tests, memory-safety reasoning, concurrency options, complexity analysis and worked examples. Develop every section in detail.'),
        dict(name='distributed-cache-1024', prompt='Write a detailed technical chapter of at least 2500 words explaining cache consistency in distributed systems. Cover invalidation, TTL, write-through and write-back, replication, failover, race conditions, versioning, concrete timelines, pseudocode, monitoring and operational trade-offs. Explain every topic with multiple examples.'),
    ]
    reference = None
    if PLAN.get('reference_root'):
        path = Path(PLAN['reference_root']) / 'request-result.json'
        if path.is_symlink() or path.stat().st_size > 1024**2:
            raise RuntimeError('bounded reference completion receipt differs')
        reference = json.loads(path.read_text())
        if reference['phase'] != 'passed' or not reference['native_tool_roundtrip_passed']:
            raise RuntimeError('exact off-profile long-output reference must pass first')
    STATE.update(phase='warming', context_tokens=1048576, max_output_tokens=1024,
                 warmup_requests=2, measured_requests=8, one_active_request=True)
    save()
    hashes = {}
    measured = []
    for number in range(10):
        case = cases[number % 2]
        warmup = number < 2
        if number == 2:
            STATE['phase'] = 'measuring'
            save()
            block_started = time.monotonic()
        row = ask(('warmup-' if warmup else 'measured-') + str(number) + '-' + case['name'],
                  dict(messages=[dict(role='user', content=case['prompt'])], max_tokens=1024))
        if row['usage']['completion_tokens'] != 1024 or row['finish_reason'] != 'length' or row['tool_calls'] or row['ttft_seconds'] is None:
            raise RuntimeError('long-output case must emit the matched1024 tokens completely')
        if row['usage']['prompt_tokens'] >= 512:
            raise RuntimeError('short-input decode comparison prompt bound exceeded')
        row.update(case=case['name'], warmup=warmup)
        wanted = hashes.setdefault(case['name'], row['output_sha256'])
        if row['output_sha256'] != wanted:
            raise RuntimeError('same-profile deterministic long output changed between repetitions')
        if reference and row['output_sha256'] != reference['reference_output_sha256'][case['name']]:
            raise RuntimeError('cumulative draft changed a matched greedy long output')
        if not warmup:
            measured.append(row)
        save()
    wall = time.monotonic() - block_started
    outputs = sum(row['usage']['completion_tokens'] for row in measured)
    interval_tokens = sum(row['usage']['completion_tokens'] - 1 for row in measured)
    interval_seconds = sum(row['first_to_last_text_seconds'] for row in measured)
    STATE.update(phase='passed', measured_completion_tokens=outputs, reference_output_sha256=hashes,
        exact_long_output_reference_matched=reference is not None, long_summary=dict(
            requests=len(measured), completion_tokens=outputs, wall_seconds=wall,
            api_wall_completion_tok_s=outputs / wall,
            stream_interval_tok_s_estimate=interval_tokens / interval_seconds,
            ttft_p50_seconds=statistics.median(row['ttft_seconds'] for row in measured),
            ttft_min_seconds=min(row['ttft_seconds'] for row in measured),
            ttft_max_seconds=max(row['ttft_seconds'] for row in measured)), finished_unix=time.time())
    save()
except BaseException as error:
    STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
    save()
    raise
