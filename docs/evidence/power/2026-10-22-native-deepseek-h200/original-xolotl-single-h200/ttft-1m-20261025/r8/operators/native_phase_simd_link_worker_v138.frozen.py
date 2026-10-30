"""Reproduce native9a684, then link trace-only and exact SIMD/trace variants.

No model requests are executed. Every unchanged archive member is compared.
"""
import hashlib
import json
import os
import shutil
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
P = json.loads((ROOT / 'native-link-plan.json').read_text())
STATE = dict(unix=time.time(), phase='preflight', gpu_math_executed=False,
             gpu_qualified=False, performance_claimed=False, variants=[])

def save():
    temp = ROOT / 'native-link-result.tmp'
    temp.write_text(json.dumps(STATE, indent=2) + '\n')
    temp.replace(ROOT / 'native-link-result.json')

def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as source:
        while block := source.read(4 * 1024**2):
            digest.update(block)
    return digest.hexdigest()

def run(args, label, cwd=None):
    STATE.update(phase='linking', current=label)
    save()
    with (ROOT / (label + '.log')).open('xb') as output:
        q = subprocess.run(args, cwd=cwd or P['link_cwd'], stdout=output, stderr=output, timeout=45)
    if q.returncode:
        raise RuntimeError('controlled native variant link failed: ' + label)

try:
    os.umask(0o077)
    os.environ['CUDA_VISIBLE_DEVICES'] = ''
    save()
    if ROOT.resolve() != ROOT or ROOT.stat().st_uid != 10250 or os.getuid() != 10250:
        raise RuntimeError('exact variant build owner differs')
    proof = json.loads((ROOT.parent / 'resource-result.json').read_text())
    old_service = json.loads((Path(P['prior_service_root']) / 'service-longctx-v1-result.json').read_text())
    if (proof['phase'] != 'passed' or time.time() + 230 > proof['leaseEndsUnix']
            or len(proof['gpus']) != 1 or proof['gpus'][0]['uuid'] != P['gpu_uuid']
            or P['gpu_uuid'] == P['formal_gpu_uuid'] or old_service['phase'] != 'stopped'
            or not old_service.get('api_reaped')):
        raise RuntimeError('finite independent idle native build proof differs')
    trace = json.loads((Path(P['trace_cpu_root']) / 'build-result.json').read_text())
    simd = json.loads((Path(P['simd_cpu_root']) / 'cpu-result.json').read_text())
    if trace['phase'] != 'compiled' or not trace['control_object_byte_equal']:
        raise RuntimeError('exact trace host object must pass controlled compilation')
    if (simd['phase'] != 'compiled' or not simd['original_object_byte_equal'] or not simd['default0_object_byte_equal']
            or not simd['bitwise_decoder_qualified'] or len(simd['tests']) != 3
            or any(row['result']['scaled_value_cases'] != 786432 for row in simd['tests'])):
        raise RuntimeError('full SIMD bitwise and sanitizer proof must pass before native link')
    for row in P['inputs']:
        path = Path(row['path'])
        if path.is_symlink() or path.stat().st_uid != 10250 or path.stat().st_size != row['bytes'] or sha(path) != row['sha256']:
            raise RuntimeError('exact controlled native input differs: ' + path.name)
    control = list(P['control_link'])
    if control.count(P['original_generator']) != 1 or control.count(P['dense_archive']) != 1 or control.count('-o') != 1:
        raise RuntimeError('exact native9a684 substitution positions differ')
    control[control.index('-o') + 1] = str(ROOT / 'ds41_generate.control9a')
    run(control, 'native9a-control')
    if sha(ROOT / 'ds41_generate.control9a') != P['control_binary_sha256']:
        raise RuntimeError('native9a684 control did not reproduce byte-for-byte')
    STATE['control_binary_byte_equal'] = True
    traced = list(control)
    traced[traced.index(P['original_generator'])] = trace['candidate_object']['path']
    traced[traced.index('-o') + 1] = str(ROOT / 'ds41_generate.trace_control')
    run(traced, 'trace-control')
    original = Path(P['dense_archive'])
    archive = ROOT / 'libds41_dense.simd.a'
    shutil.copyfile(original, archive)
    run(['ar', 'rD', str(archive), simd['candidate_object']['path']], 'replace-engram-member', ROOT)
    def members(path):
        return subprocess.run(['ar', 't', str(path)], capture_output=True, text=True, check=True, timeout=5).stdout.splitlines()
    names = members(original)
    if names != members(archive) or names.count('ds41_engram.cpp.o') != 1:
        raise RuntimeError('dense archive member order or identity changed')
    verified = []
    for name in names:
        left = subprocess.run(['ar', 'p', str(original), name], capture_output=True, check=True, timeout=5).stdout
        right = subprocess.run(['ar', 'p', str(archive), name], capture_output=True, check=True, timeout=5).stdout
        if name == 'ds41_engram.cpp.o':
            if hashlib.sha256(right).hexdigest() != simd['candidate_object']['sha256']:
                raise RuntimeError('replacement SIMD member differs from proved CPU object')
        elif left != right:
            raise RuntimeError('another model archive member changed')
        verified.append(dict(member=name, sha256=hashlib.sha256(right).hexdigest(), unchanged=name != 'ds41_engram.cpp.o'))
    STATE['dense_members'] = verified
    candidate = list(traced)
    candidate[candidate.index(P['dense_archive'])] = str(archive)
    candidate[candidate.index('-o') + 1] = str(ROOT / 'ds41_generate.trace_simd')
    run(candidate, 'trace-simd-candidate')
    env = {key: value for key, value in os.environ.items() if not key.startswith(('STRATA_', 'ASTER_XQ_'))}
    env['CUDA_VISIBLE_DEVICES'] = ''
    for label in ['trace_control', 'trace_simd']:
        executable = ROOT / ('ds41_generate.' + label)
        rejected = []
        for value in ['', '2', '-1', 'true', '01', ' 1', '0 ', 'NaN']:
            q = subprocess.run([str(executable), '--help'], env=dict(env, ASTER_XQ_PREFILL_PHASE_TRACE=value),
                               capture_output=True, text=True, timeout=5)
            if q.returncode != 2 or q.stderr.strip() != 'ASTER_XQ_PREFILL_PHASE_TRACE must be 0 or 1':
                raise RuntimeError('invalid prefill trace mode was not refused before backend startup')
            rejected.append(value)
        STATE['variants'].append(dict(profile=label, path=str(executable), bytes=executable.stat().st_size,
                                      sha256=sha(executable), invalid_modes_refused=len(rejected)))
        save()
    STATE.update(phase='linked', only_engram_member_changed=True, model_math_executed=False,
                 routing_changed=False, acceptance_changed=False, finished_unix=time.time())
except BaseException as error:
    STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
    raise
finally:
    save()
