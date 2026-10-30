"""Sequential finite API qualification on the isolated V44 service, public data only."""
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PLAN = json.loads((ROOT / 'suite-plan.json').read_text())
STATE = dict(schema='aster-xolotl-stable-edge-suite-v59', unix=time.time(),
             phase='preflight', stages=[], formal_promoted=False)


def save():
    temp = ROOT / 'suite-result.tmp'
    temp.write_text(json.dumps(STATE, indent=2) + '\n')
    temp.replace(ROOT / 'suite-result.json')


def require_service():
    service = Path(PLAN['service_root'])
    if service.resolve() != service or service.stat().st_uid != 10250:
        raise RuntimeError('exact isolated service owner differs')
    receipt = json.loads((service / 'service-longctx-v1-result.json').read_text())
    if (receipt['phase'] != 'serving' or receipt['gpu_uuid'] != PLAN['gpu_uuid']
            or receipt['binary_sha256'] != PLAN['binary_sha256']
            or receipt['context_tokens'] != 1048576):
        raise RuntimeError('exact off-repeat service identity differs')
    return receipt


try:
    os.umask(0o077)
    save()
    for stage, budget in [('remainder', 1100)]:
        receipt = require_service()
        if time.time() + budget + 120 > receipt['lease_ends_unix']:
            raise RuntimeError('finite exact service lease insufficient for next stage')
        folder = ROOT / stage
        STATE.update(phase='running', current_stage=stage, stage_started_unix=time.time())
        save()
        with (folder / 'request-worker.log').open('xb') as log:
            process = subprocess.Popen([sys.executable, '-u', str(folder / 'request-worker.py')],
                                       cwd=folder, stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                       start_new_session=True)
            stat = Path('/proc/' + str(process.pid) + '/stat').read_text().split(') ', 1)[1].split()
            STATE['stage_process'] = dict(pid=process.pid, start_ticks=int(stat[19]),
                                          uid=os.getuid(), budget_seconds=budget)
            save()
            try:
                returncode = process.wait(timeout=budget)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=10)
                raise RuntimeError(stage + ' exhausted finite stage budget')
        path = folder / 'request-result.json'
        if path.is_symlink() or path.stat().st_size > 1024**2:
            raise RuntimeError('bounded public stage receipt differs')
        result = json.loads(path.read_text())
        if returncode != 0 or result['phase'] != 'passed':
            raise RuntimeError(stage + ' qualification failed')
        if stage == 'remainder':
            if result['usage']['prompt_tokens'] != 132097 or result['content'].strip() != '42':
                raise RuntimeError('strict132097 remainder gate differs')
        else:
            if (result['fixture']['actual_prompt_tokens'] != 1047544 or not result['full_1m_length_qualified']
                    or not result['three_position_retrieval_passed'] or not result['full1m_cache_restore_passed']
                    or not result['long_output_qualified'] or not result['exact_restored_long_output_matched']
                    or result['tests'][1]['name'] != 'cold-full1m-1024'
                    or result['tests'][1]['usage']['prompt_tokens_details']['cached_tokens'] != 0
                    or result['tests'][1]['usage']['completion_tokens'] != 1024
                    or result['measured_requests'] != 16 or len(result['tests']) != 20
                    or [x['concurrency'] for x in result['levels']] != [1, 2]):
                raise RuntimeError('actual cold1M/1024-output/retrieval/restore/finite-load gate differs')
        STATE['stages'].append(dict(name=stage, phase='passed', finished_unix=time.time()))
        save()
    STATE.update(phase='passed', finished_unix=time.time())
    save()
except BaseException as error:
    STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
    save()
    raise
