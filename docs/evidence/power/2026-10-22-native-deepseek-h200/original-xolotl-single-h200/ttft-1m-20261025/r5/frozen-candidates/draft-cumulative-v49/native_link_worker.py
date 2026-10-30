"""Link only a new host generator into the exact qualified CUDA/MoE/dense input set."""
import hashlib
import json
import os
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PLAN = json.loads((ROOT / 'native-link-plan.json').read_text())
STATE = dict(unix=time.time(), phase='preflight', gpu_qualified=False, performance_claimed=False)


def save():
    temporary = ROOT / 'native-link-result.tmp'
    temporary.write_text(json.dumps(STATE, indent=2) + '\n')
    temporary.replace(ROOT / 'native-link-result.json')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(args, name):
    STATE['step'] = name
    save()
    with (ROOT / (name + '.log')).open('xb') as log:
        result = subprocess.run(args, cwd=PLAN['link_cwd'], stdout=log, stderr=log, timeout=40)
    if result.returncode:
        raise RuntimeError(name + ' failed')


try:
    os.umask(0o077)
    os.environ['CUDA_VISIBLE_DEVICES'] = ''
    save()
    if ROOT.resolve() != ROOT or ROOT.stat().st_uid != 10250 or os.getuid() != 10250:
        raise RuntimeError('exact native link owner differs')
    proof = json.loads((ROOT.parent / 'resource-result.json').read_text())
    if proof['phase'] != 'passed' or time.time() + 180 > proof['leaseEndsUnix']:
        raise RuntimeError('finite native link resource lease insufficient')
    cpu = json.loads((Path(PLAN['cpu_root']) / 'generator-cpu-build-result.json').read_text())
    if cpu['phase'] != 'compiled' or not cpu['original_generator_object_byte_equal']:
        raise RuntimeError('exact controlled host generator compilation required')
    for row in PLAN['inputs']:
        path = Path(row['path'])
        if path.is_symlink() or path.stat().st_size != row['bytes'] or sha(path) != row['sha256']:
            raise RuntimeError('qualified native link input differs')
    control = list(PLAN['control_link'])
    if control.count(PLAN['original_host_object']) != 1 or control.count('-o') != 1:
        raise RuntimeError('exact native host object substitution differs')
    control[control.index('-o') + 1] = str(ROOT / 'ds41_generate.control')
    run(control, 'exact-v44-control-link')
    if sha(ROOT / 'ds41_generate.control') != PLAN['control_binary_sha256']:
        raise RuntimeError('native link does not reproduce qualified V44 byte-for-byte')
    STATE['control_binary_byte_equal'] = True
    candidate = list(control)
    candidate[candidate.index(PLAN['original_host_object'])] = PLAN['candidate_host_object']
    candidate[candidate.index('-o') + 1] = str(ROOT / 'ds41_generate')
    run(candidate, 'cumulative-candidate-link')
    env = {key: value for key, value in os.environ.items() if not key.startswith(('STRATA_', 'ASTER_XQ_'))}
    env['CUDA_VISIBLE_DEVICES'] = ''
    rejected = []
    for value in ['', '1', '0.10', 'true', 'NaN', 'inf', ' 0.1', '0.1 ']:
        result = subprocess.run([str(ROOT / 'ds41_generate'), '--help'], env=dict(env, ASTER_XQ_DSPARK_CUM=value),
                                capture_output=True, text=True, timeout=5)
        if result.returncode != 2 or result.stderr.strip() != 'ASTER_XQ_DSPARK_CUM must be 0, 0.1, 0.25, or 0.5':
            raise RuntimeError('invalid cumulative mode did not refuse before backend startup')
        rejected.append(value)
    STATE.update(phase='linked', binary=dict(path=str(ROOT / 'ds41_generate'),
        bytes=(ROOT / 'ds41_generate').stat().st_size, sha256=sha(ROOT / 'ds41_generate')),
        invalid_startup_modes_rejected=len(rejected), target_acceptance_changed=False,
        routing_changed=False, gpu_math_executed=False, finished_unix=time.time())
except BaseException as error:
    STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
    raise
finally:
    save()
