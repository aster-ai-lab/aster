"""Reproduce native34f, replace only the checked V94 generator object, and reject invalid flags."""
import hashlib
import json
import os
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PLAN = json.loads((ROOT / 'native-link-plan.json').read_text())
STATE = dict(unix=time.time(), phase='preflight', gpu_math_executed=False,
             gpu_qualified=False, performance_claimed=False)


def save():
    p = ROOT / 'native-link-result.tmp'
    p.write_text(json.dumps(STATE, indent=2) + '\n')
    p.replace(ROOT / 'native-link-result.json')


def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        while block := f.read(4 * 1024 * 1024):
            h.update(block)
    return h.hexdigest()


def run(command, label):
    STATE.update(phase='linking', step=label)
    save()
    with (ROOT / (label + '.log')).open('xb') as log:
        p = subprocess.run(command, cwd=PLAN['link_cwd'], stdout=log, stderr=log, timeout=50)
    if p.returncode:
        raise RuntimeError('controlled link failed: ' + label)


try:
    os.umask(0o077)
    os.environ['CUDA_VISIBLE_DEVICES'] = ''
    save()
    if ROOT.resolve() != ROOT or ROOT.stat().st_uid != 10250 or os.getuid() != 10250:
        raise RuntimeError('exact native-link artifact owner differs')
    if ROOT.parent.name != PLAN['resource_directory']:
        raise RuntimeError('exact optimization resource root differs')
    proof = json.loads((ROOT.parent / 'resource-result.json').read_text())
    if (proof['phase'] != 'passed' or time.time() + 220 > proof['leaseEndsUnix']
            or len(proof['gpus']) != 1 or proof['gpus'][0]['uuid'] != PLAN['expected_gpu_uuid']
            or PLAN['expected_gpu_uuid'] == PLAN['independent_formal_gpu_uuid']):
        raise RuntimeError('finite independent optimization GPU proof differs')
    cpu = json.loads((Path(PLAN['cpu_root']) / 'build-result.json').read_text())
    if (cpu['phase'] != 'compiled' or not cpu['control_object_byte_equal']
            or cpu['candidate_object']['path'] != PLAN['candidate_host_object']):
        raise RuntimeError('exact V94 controlled CPU compilation must be complete')
    if not cpu.get('upstream_pool_suite_passed') or [(r['mode'],r['contracts'],r['exit']) for r in cpu['unit_tests']] != [('release',113,0),('asan_ubsan',113,0)]:
        raise RuntimeError('all exact pool/seed CPU and sanitizer contracts must pass')
    for row in PLAN['inputs']:
        p = Path(row['path'])
        if p.is_symlink() or p.stat().st_uid != 10250 or p.stat().st_size != row['bytes'] or sha(p) != row['sha256']:
            raise RuntimeError('qualified native link input differs: ' + p.name)
    control = list(PLAN['control_link'])
    if control.count(PLAN['original_host_object']) != 1 or control.count('-o') != 1:
        raise RuntimeError('single qualified generator substitution required')
    control[control.index('-o') + 1] = str(ROOT / 'ds41_generate.control')
    run(control, 'nativec96-control')
    if sha(ROOT / 'ds41_generate.control') != PLAN['control_binary_sha256']:
        raise RuntimeError('exact native34f control binary did not reproduce')
    STATE['control_binary_byte_equal'] = True
    candidate = list(control)
    candidate[candidate.index(PLAN['original_host_object'])] = PLAN['candidate_host_object']
    candidate[candidate.index('-o') + 1] = str(ROOT / 'ds41_generate')
    run(candidate, 'pool-seed-candidate')
    env = {k: v for k, v in os.environ.items() if not k.startswith(('STRATA_', 'ASTER_XQ_'))}
    env['CUDA_VISIBLE_DEVICES'] = ''
    rejected = []
    for value in ['', '2', '-1', 'true', '01', ' 1', '0 ', 'NaN']:
        p = subprocess.run([str(ROOT / 'ds41_generate'), '--help'],
                           env=dict(env, ASTER_XQ_POOL_DRAFT_SEED=value),
                           capture_output=True, text=True, timeout=5)
        if p.returncode != 2 or p.stderr.strip() != 'ASTER_XQ_POOL_DRAFT_SEED must be 0 or 1':
            raise RuntimeError('invalid admission-seed flag was not refused before backend startup')
        rejected.append(value)
    binary = ROOT / 'ds41_generate'
    STATE.update(phase='linked', invalid_startup_modes_rejected=len(rejected),
                 binary=dict(path=str(binary), bytes=binary.stat().st_size, sha256=sha(binary)),
                 target_acceptance_changed=False, routing_changed=False, finished_unix=time.time())
except BaseException as error:
    STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
    raise
finally:
    save()
