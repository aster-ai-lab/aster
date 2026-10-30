"""Real full-window, fixed public retrieval load at concurrency one and two."""
import concurrent.futures, hashlib, json, os, statistics, threading, time, urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent
P = json.loads((ROOT / 'request-plan.json').read_text())
service = Path(P['service_root'])
full = Path(P['full_qualification_root'])
prior = json.loads((full / 'request-result.json').read_text())
lock = threading.Lock()
STATE = dict(schema='aster-xolotl-fullwindow-reuse-load-v20', unix=time.time(), phase='preflight',
             scope='Same actual1048504-token public prompt, max64 output, exact three-record JSON; cache-hit load, not cold concurrency or long output.',
             tests=[], levels=[], full_1m_cold_qualification_source=str(full), performance_300_claimed=False)

def save():
    temp = ROOT / 'request-result.tmp'
    temp.write_text(json.dumps(STATE, indent=2) + '\n')
    temp.replace(ROOT / 'request-result.json')

def percentile(values, probability):
    values = sorted(values)
    index = (len(values) - 1) * probability
    lo = int(index)
    return values[lo] + (values[min(lo + 1, len(values) - 1)] - values[lo]) * (index - lo)

try:
    os.umask(0o077)
    save()
    if prior['phase'] != 'passed' or not prior.get('full1m_cache_restore_passed'):
        raise RuntimeError('actual full-window cold/retrieval/restore gate must pass first')
    raw_fixture = (full / 'public-fixture.txt').read_bytes()
    if hashlib.sha256(raw_fixture).hexdigest() != prior['prompt_sha256']:
        raise RuntimeError('exact qualified full-window public fixture differs')
    target = prior['actual_prompt_tokens']
    if target != 1048504 or prior['context_safety_tokens'] != 8 or prior['max_new_tokens'] != 64:
        raise RuntimeError('exact full-window input/output/safety accounting differs')
    key = (service / 'credentials/api-key.txt').read_text().strip()
    payload = dict(model='deepseek-v4.1-flash-1017-xq', messages=[dict(role='user', content=raw_fixture.decode())],
                   max_tokens=64, temperature=0, seed=1, reasoning_effort='none', stream=True,
                   stream_options=dict(include_usage=True))
    encoded = json.dumps(payload).encode()
    if len(encoded) > 8 * 1024**2:
        raise RuntimeError('bounded full-window public payload exceeded')
    STATE.update(actual_prompt_tokens=target, max_output_tokens=64, context_tokens=1048576, context_safety_tokens=8,
                 expected=prior['expected'], prompt_sha256=prior['prompt_sha256'], payload_bytes=len(encoded))
    save()

    def ask(name):
        request = urllib.request.Request(P['base_url'] + '/chat/completions', data=encoded,
                                         headers={'Authorization': 'Bearer ' + key, 'Content-Type': 'application/json'})
        start = time.monotonic()
        first = last = None
        text = ''
        usage = finish = None
        done = False
        size = 0
        with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request, timeout=130) as response:
            status = response.status
            for line in response:
                size += len(line)
                if size > 1024**2 or time.monotonic() - start > 120:
                    raise RuntimeError(name + ' exceeded response/time budget')
                if not line.startswith(b'data:'):
                    continue
                value = line[5:].strip()
                if value == b'[DONE]':
                    done = True
                    break
                data = json.loads(value)
                for choice in data.get('choices', []):
                    part = choice.get('delta', {}).get('content') or ''
                    if part:
                        last = time.monotonic()
                        first = first or last
                        text += part
                    finish = choice.get('finish_reason') or finish
                usage = data.get('usage') or usage
                if len(text) > 4096:
                    raise RuntimeError('bounded public retrieval output exceeded')
        if status != 200 or not done or not finish or usage is None or first is None:
            raise RuntimeError(name + ' did not finish completely')
        if usage['prompt_tokens'] != target or usage.get('prompt_tokens_details', {}).get('cached_tokens', 0) < target - 8:
            raise RuntimeError(name + ' full-window cache accounting failed')
        if json.loads(text) != prior['expected']:
            raise RuntimeError(name + ' public three-position retrieval failed')
        row = dict(name=name, http_status=status, done=done, finish_reason=finish, content=text, usage=usage,
                   seconds=time.monotonic() - start, ttft_seconds=first - start,
                   first_to_last_text_seconds=last - first)
        with lock:
            STATE['tests'].append(row)
            save()
        return row

    STATE['phase'] = 'warming'
    save()
    for index in range(2):
        ask('warmup-' + str(index))
    for concurrency in [1, 2]:
        with lock:
            STATE.update(phase='measuring', current_concurrency=concurrency)
            save()
        begin = time.monotonic()
        with concurrent.futures.ThreadPoolExecutor(max_workers=concurrency) as pool:
            futures = [pool.submit(ask, 'c' + str(concurrency) + '-' + str(index)) for index in range(16)]
            rows = [future.result() for future in futures]
        wall = time.monotonic() - begin
        outputs = sum(row['usage']['completion_tokens'] for row in rows)
        stream_seconds = sum(row['first_to_last_text_seconds'] for row in rows)
        level = dict(concurrency=concurrency, requests=len(rows), completion_tokens=outputs, wall_seconds=wall,
                     aggregate_end_to_end_completion_tok_s=outputs / wall,
                     ttft_p50_seconds=statistics.median(row['ttft_seconds'] for row in rows),
                     ttft_p95_seconds=percentile([row['ttft_seconds'] for row in rows], .95),
                     request_p95_seconds=percentile([row['seconds'] for row in rows], .95),
                     per_stream_interval_tok_s_estimate=outputs / stream_seconds if stream_seconds else None)
        with lock:
            STATE['levels'].append(level)
            save()
    STATE.update(phase='passed', measured_requests=32, warmup_requests=2, finished_unix=time.time())
    save()
except BaseException as error:
    with lock:
        STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
        save()
    raise
