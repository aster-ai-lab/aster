"""Actual full-window1024-output qualification and finite cache-hit load, public data only."""
import concurrent.futures
import hashlib
import json
import os
import socket
import statistics
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path
from xolotl_fullwindow_long_fixture_v54 import INPUT, OUTPUT, check_completion, check_record_prefix

ROOT = Path(__file__).resolve().parent
PLAN = json.loads((ROOT / 'request-plan.json').read_text())
SERVICE = Path(PLAN['service_root'])
FIXTURE = Path(PLAN['fixture_root'])
LOCK = threading.RLock()
STATE = dict(schema='aster-xolotl-fullwindow-long-output-v54', unix=time.time(), phase='preflight',
             client=socket.gethostname(), tests=[], levels=[], active_requests={},
             scope='Actual1047544 unique-public-source input plus1024 output and8 safety; no corpus cycling; unchanged exact cold/restore/c1/c2 gates.',
             full_1m_length_qualified=False, long_output_qualified=False, performance_300_claimed=False)


def save():
    with LOCK:
        raw = (json.dumps(STATE, indent=2) + '\n').encode()
        if len(raw) > 1024**2:
            raise RuntimeError('bounded public full-window long-output receipt exceeded')
        temporary = ROOT / 'request-result.tmp'
        temporary.write_bytes(raw)
        temporary.replace(ROOT / 'request-result.json')


def ask(name, text, max_tokens=OUTPUT, budget=240):
    key = (SERVICE / 'credentials/api-key.txt').read_text().strip()
    raw = json.dumps(dict(model='deepseek-v4.1-flash-1017-xq',
        messages=[dict(role='user', content=text)], max_tokens=max_tokens, temperature=0, seed=1,
        reasoning_effort='none', stream=True, stream_options=dict(include_usage=True))).encode()
    if len(raw) > 8 * 1024**2:
        raise RuntimeError('public full-window payload byte bound exceeded')
    request = urllib.request.Request(PLAN['base_url'] + '/chat/completions', data=raw,
        headers={'Authorization': 'Bearer ' + key, 'Content-Type': 'application/json'})
    start = time.monotonic()
    first = last = None
    usage = finish = None
    content = ''
    received = chunks = 0
    done = False
    with LOCK:
        STATE['active_requests'][name] = dict(started_unix=time.time(), state='prefilling')
        save()
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    try:
        response = opener.open(request, timeout=budget)
    except urllib.error.HTTPError as error:
        text = error.read(4097)
        if len(text) > 4096:
            raise RuntimeError('bounded public full-window HTTP error exceeded') from error
        raise RuntimeError(name + ' HTTP' + str(error.code) + ': '
                           + text.decode(errors='replace').replace(key, '[REDACTED]')) from error
    with response:
        status = response.status
        for line in response:
            received += len(line)
            if received > 2 * 1024**2 or time.monotonic() - start > budget:
                raise RuntimeError(name + ' exceeded finite response/time budget')
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
                if delta.get('tool_calls'):
                    raise RuntimeError('unexpected tool call in plain public retrieval/tutorial')
                part = delta.get('content') or ''
                if part:
                    last = time.monotonic()
                    if first is None:
                        first = last
                        with LOCK:
                            STATE['active_requests'][name].update(state='streaming', first_text_unix=time.time())
                            save()
                    chunks += 1
                    content += part
                finish = choice.get('finish_reason') or finish
            usage = item.get('usage') or usage
            if len(content.encode()) > 131072:
                raise RuntimeError('full-window public completion byte bound exceeded')
    row = dict(name=name, status=status, done=done, finish_reason=finish, content=content, usage=usage,
               output_sha256=hashlib.sha256(content.encode()).hexdigest(), seconds=time.monotonic() - start,
               ttft_seconds=first - start if first is not None else None,
               first_to_last_text_seconds=last - first if first is not None else None,
               text_chunks=chunks, response_bytes=received)
    with LOCK:
        STATE['active_requests'].pop(name)
        STATE['tests'].append(row)
        save()
    if status != 200 or not done or not finish or usage is None or first is None:
        raise RuntimeError(name + ' did not finish completely')
    return row


def percentile(values, probability):
    values = sorted(values)
    index = (len(values) - 1) * probability
    low = int(index)
    return values[low] + (values[min(low + 1, len(values) - 1)] - values[low]) * (index - low)


try:
    os.umask(0o077)
    save()
    if FIXTURE.resolve() != FIXTURE or FIXTURE.stat().st_uid != 10250:
        raise RuntimeError('exact public full-window fixture owner differs')
    metadata = json.loads((FIXTURE / 'fixture-metadata.json').read_text())
    raw = (FIXTURE / 'public-fixture.txt').read_bytes()
    if (len(raw) != metadata['prompt_bytes'] or hashlib.sha256(raw).hexdigest() != PLAN['fixture_sha256']
            or metadata['prompt_sha256'] != PLAN['fixture_sha256']
            or metadata['prompt_id_sha256'] != PLAN['fixture_id_sha256']
            or metadata['actual_prompt_tokens'] != INPUT):
        raise RuntimeError('CPU-qualified exact full-window public fixture differs')
    text = raw.decode()
    STATE.update(fixture=metadata, exact_greedy_restore_required=True)
    save()
    # Separate explicit public control; unique code corpus is never rewritten or reduced.
    tiny = 'Public three-record long-output format control.\nBEGIN_RECORD_CODE: 731927\nMIDDLE_RECORD_CODE: 408563\n\nEND_RECORD_CODE: 962814\nOn the first line return only the three record codes as a plain JSON object with keys begin, middle, end and integer values. Do not use markdown fences. After that line, write a detailed technical chapter of at least 2500 words explaining cache consistency in distributed systems. Cover invalidation, TTL, write-through and write-back, replication, failover, race conditions, versioning, concrete timelines, pseudocode, monitoring and operational trade-offs. Explain every topic in detail with multiple examples. Do not repeat filler words.'
    if len(tiny) > 8192:
        raise RuntimeError('declared public fixture filler removal differs')
    STATE['phase'] = 'short_format_gate'
    save()
    preview = ask('short-three-records-1024', tiny)
    if (preview['usage']['prompt_tokens'] >= 512 or preview['usage']['completion_tokens'] != OUTPUT
            or preview['finish_reason'] != 'length'):
        raise RuntimeError('cheap complete1024-token format gate failed')
    check_record_prefix(preview['content'])
    STATE.update(phase='cold_fullwindow_long', short_format_gate_passed=True)
    save()
    cold = ask('cold-full1m-1024', text, budget=6000)
    digest = check_completion(cold, metadata, cold=True)
    STATE.update(full_1m_length_qualified=True, long_output_qualified=True,
                 three_position_retrieval_passed=True, cold_output_sha256=digest,
                 phase='isolation_and_restore')
    save()
    control = ask('interleaved-isolation-53', 'Calculate 19 + 34. Reply with only the integer answer.', max_tokens=16)
    if control['content'].strip() != '53':
        raise RuntimeError('interleaved full-window conversation isolation failed')
    restored = ask('restored-full1m-1024', text)
    check_completion(restored, metadata, cold=False, reference_sha256=digest)
    STATE.update(full1m_cache_restore_passed=True, exact_restored_long_output_matched=True, phase='cache_hit_load')
    save()
    requests = PLAN['load_requests_per_level']
    if requests != 8:
        raise RuntimeError('finite fixed full-window load count differs')

    def measured(name):
        row = ask(name, text)
        check_completion(row, metadata, cold=False, reference_sha256=digest)
        return row

    for concurrency in [1, 2]:
        with LOCK:
            STATE['current_concurrency'] = concurrency
            save()
        start = time.monotonic()
        with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as executor:
            futures = [executor.submit(measured, 'c' + str(concurrency) + '-' + str(i)) for i in range(requests)]
            rows = [future.result() for future in futures]
        wall = time.monotonic() - start
        outputs = sum(row['usage']['completion_tokens'] for row in rows)
        interval = sum(row['first_to_last_text_seconds'] for row in rows)
        level = dict(concurrency=concurrency, requests=requests, completion_tokens=outputs,
                     wall_seconds=wall, aggregate_end_to_end_completion_tok_s=outputs / wall,
                     ttft_p50_seconds=statistics.median(row['ttft_seconds'] for row in rows),
                     ttft_p95_seconds=percentile([row['ttft_seconds'] for row in rows], .95),
                     request_p95_seconds=percentile([row['seconds'] for row in rows], .95),
                     per_stream_interval_tok_s_estimate=(outputs - len(rows)) / interval)
        with LOCK:
            STATE['levels'].append(level)
            save()
    STATE.update(phase='passed', measured_requests=16, measured_completion_tokens=16384,
                 cache_hit_load_passed=True, finished_unix=time.time())
    save()
except BaseException as error:
    with LOCK:
        STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
        save()
    raise
