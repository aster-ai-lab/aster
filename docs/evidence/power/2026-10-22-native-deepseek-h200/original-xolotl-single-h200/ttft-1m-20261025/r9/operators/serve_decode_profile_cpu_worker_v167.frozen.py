"""Compile a reproduced 9a684 host object and bounded default-off prefill trace.

The process runs on jump CPUs with CUDA hidden. It never links or runs a model.
"""
import hashlib
import json
import os
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PLAN = json.loads((ROOT / 'build-plan.json').read_text())
STATE = dict(unix=time.time(), phase='preflight', runs=[], gpu_executed=False,
             model_executed=False, speed_gain_claimed=False)

def save():
    p = ROOT / 'build-result.tmp'
    p.write_text(json.dumps(STATE, indent=2) + '\n')
    p.replace(ROOT / 'build-result.json')

def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()

try:
    os.umask(0o077)
    save()
    if os.getuid() != 10250 or ROOT.resolve() != ROOT or ROOT.stat().st_uid != 10250:
        raise RuntimeError('exact CPU compile owner differs')
    parent = Path(PLAN['parent_cpu_root'])
    old = json.loads((parent / 'build-result.json').read_text())
    if old['phase'] != 'compiled' or not old['control_object_byte_equal']:
        raise RuntimeError('qualified parent host object must have completed')
    if old['candidate_object']['sha256'] != '8ab829c4157968935bb4f95b31b73ebd1b60c6fb3fa6e6db144c30fc496c3715':
        raise RuntimeError('exact 9a684 host object provenance differs')
    original = parent / 'ds41_generate.cpp'
    if sha(original) != PLAN['parent_source_sha256']:
        raise RuntimeError('exact unchanged 9a684 source differs')
    for row in PLAN['inputs']:
        p = ROOT / row['file']
        if p.is_symlink() or p.stat().st_size != row['bytes'] or sha(p) != row['sha256']:
            raise RuntimeError('frozen prefill observation input differs')
    args = old['runs'][-1]['command']
    if args[:4] != ['nice', '-n', '19', '/usr/bin/c++']:
        raise RuntimeError('exact compiler/scheduling prefix differs')
    clean = []
    i = 3
    while i < len(args):
        if args[i] in ['-o', '-c', '-MT', '-MF']:
            i += 2
            continue
        if args[i] == '-MD':
            i += 1
            continue
        clean.append(args[i])
        i += 1
    env = {k: v for k, v in os.environ.items() if not k.startswith(('STRATA_', 'ASTER_XQ_'))}
    env.update(CUDA_VISIBLE_DEVICES='', OMP_NUM_THREADS='1')
    STATE['tests'] = []
    for label, flags in [('release', ['-O3']), ('asan_ubsan', ['-O1', '-g', '-fsanitize=address,undefined', '-fno-omit-frame-pointer'])]:
        command = ['/usr/bin/c++', '-std=c++20', '-Wall', '-Wextra', '-Werror', *flags,
                   'test_serve_profile.cpp', '-o', 'test-' + label]
        result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True, timeout=30)
        (ROOT / (label + '-unit-compile.log')).write_text(result.stdout + result.stderr)
        if result.returncode:
            raise RuntimeError('profiler lifecycle unit compilation failed: ' + label)
        result = subprocess.run([str(ROOT / ('test-' + label))], cwd=ROOT, env=env, capture_output=True, text=True, timeout=20)
        (ROOT / (label + '-unit-run.log')).write_text(result.stdout + result.stderr)
        proof = json.loads(result.stdout) if result.returncode == 0 else {}
        if result.returncode or proof.get('phase') != 'passed' or proof.get('contracts') != 20015 or result.stderr:
            raise RuntimeError('profiler lifecycle acceptance failed: ' + label)
        STATE['tests'].append(dict(mode=label, result=proof))
        save()
    path = Path(PLAN['moe_object'])
    if path.is_symlink() or path.stat().st_uid != 10250 or path.stat().st_size > 16*1024**2:
        raise RuntimeError('exact original MoE object owner/bound differs')
    result = subprocess.run(['nm', '-u', str(path)], capture_output=True, text=True, check=True, timeout=15)
    if len(result.stdout) > 1048576:
        raise RuntimeError('bounded native profiler symbol observation exceeded')
    symbols = {line.split()[-1] for line in result.stdout.splitlines() if line.split()}
    if not {'cudaProfilerStart', 'cudaProfilerStop'} <= symbols:
        raise RuntimeError('existing native MoE profiler API hooks are absent')
    STATE['original_moe_profiler_CUDA_hooks_present'] = True
    save()
    for label, source in [('control', original), ('candidate', ROOT / 'ds41_generate.cpp')]:
        STATE.update(phase='compiling', current=label)
        save()
        target = ROOT / ('generator-' + label + '.o')
        command = ['nice', '-n', '19', *clean, '-o', str(target), '-c', str(source)]
        with (ROOT / (label + '-compile.log')).open('xb') as output:
            result = subprocess.run(command, cwd=ROOT, env=env, stdout=output, stderr=output, timeout=115)
        if result.returncode:
            raise RuntimeError('controlled prefill phase compilation failed: ' + label)
        row = dict(label=label, command=command,
                   object=dict(path=str(target), bytes=target.stat().st_size, sha256=sha(target)))
        STATE['runs'].append(row)
        if label == 'control':
            if row['object']['sha256'] != old['candidate_object']['sha256']:
                raise RuntimeError('exact native9a684 host control did not reproduce')
            STATE['control_object_byte_equal'] = True
        save()
    STATE.update(phase='compiled', candidate_object=STATE['runs'][-1]['object'],
                 gpu_qualified=False, performance_claimed=False, finished_unix=time.time())
except BaseException as error:
    STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
    raise
finally:
    save()
