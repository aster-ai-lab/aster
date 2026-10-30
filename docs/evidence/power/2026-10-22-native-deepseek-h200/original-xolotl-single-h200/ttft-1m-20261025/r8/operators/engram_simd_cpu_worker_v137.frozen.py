"""Finite CPU proof for real Engram dequantization and exact native object lineage."""
import hashlib
import json
import os
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PLAN = json.loads((ROOT / 'build-plan.json').read_text())
STATE = dict(unix=time.time(), phase='preflight', tests=[], runs=[], gpu_executed=False,
             model_executed=False, model_speed_claimed=False)

def save():
    temp = ROOT / 'cpu-result.tmp'
    temp.write_text(json.dumps(STATE, indent=2) + '\n')
    temp.replace(ROOT / 'cpu-result.json')

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def run(args, label, env, limit):
    STATE.update(phase='executing', current=label)
    save()
    result = subprocess.run(['nice', '-n', '19', *args], cwd=ROOT, env=env,
                            capture_output=True, text=True, timeout=limit)
    (ROOT / (label + '.log')).write_text(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError('finite SIMD CPU command failed: ' + label + ': ' + result.stderr[-1500:])
    return result

try:
    os.umask(0o077)
    save()
    if os.getuid() != 10250 or ROOT.resolve() != ROOT or ROOT.stat().st_uid != 10250:
        raise RuntimeError('exact SIMD CPU owner differs')
    for row in PLAN['inputs']:
        path = ROOT / row['file']
        if path.is_symlink() or path.stat().st_size != row['bytes'] or sha(path) != row['sha256']:
            raise RuntimeError('frozen SIMD CPU source differs')
    env = {k: v for k, v in os.environ.items() if not k.startswith(('STRATA_', 'ASTER_XQ_'))}
    env.update(CUDA_VISIBLE_DEVICES='', OMP_NUM_THREADS='1', ASAN_OPTIONS='detect_leaks=1:halt_on_error=1',
               UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1')
    cpuinfo = Path('/proc/cpuinfo').read_text()
    STATE['cpu_model'] = next((line.split(':', 1)[1].strip() for line in cpuinfo.splitlines() if line.startswith('model name')), None)
    for mode, flags in [('control0', ['-O3', '-DASTER_XQ_ENGRAM_FP8_SIMD=0']),
                        ('enabled1', ['-O3', '-DASTER_XQ_ENGRAM_FP8_SIMD=1']),
                        ('sanitized1', ['-O1', '-g', '-DASTER_XQ_ENGRAM_FP8_SIMD=1', '-fsanitize=address,undefined', '-fno-omit-frame-pointer'])]:
        command = ['/usr/bin/c++', '-std=c++20', '-frounding-math', '-Wall', '-Wextra', '-Werror', *flags,
                   'fp8_simd_exact_test.cpp', '-o', mode]
        run(command, mode + '-compile', env, 35)
        result = run([str(ROOT / mode)], mode + '-test', env, 35)
        q = json.loads(result.stdout)
        if q['phase'] != 'passed' or q['scaled_value_cases'] != 786432 or not q['bitwise_equal']:
            raise RuntimeError('complete SIMD/scale/rounding/unaligned contract differs')
        if mode != 'control0' and (not q['avx2_enabled'] or q['normal_vector_cases'] != 728064 or q['scalar_fallback_cases'] != 58368):
            raise RuntimeError('actual AVX2 branch coverage is required; scalar-only execution is not its proof')
        STATE['tests'].append(dict(mode=mode, command=command, result=q, stderr_empty=not result.stderr))
        save()
    bad = subprocess.run(['/usr/bin/c++', '-std=c++20', '-DASTER_XQ_ENGRAM_FP8_SIMD=2',
                          'fp8_simd_exact_test.cpp', '-o', 'invalid2'], cwd=ROOT, env=env,
                         capture_output=True, text=True, timeout=15)
    (ROOT / 'invalid2-compile.log').write_text(bad.stdout + bad.stderr)
    if bad.returncode == 0 or 'ASTER_XQ_ENGRAM_FP8_SIMD must be 0 or 1' not in bad.stderr:
        raise RuntimeError('unknown SIMD compile mode was not refused')
    STATE['invalid_mode_refused'] = True
    old_root = Path(PLAN['original_engram_cpu_root'])
    old = json.loads((old_root / 'build-result.json').read_text())
    if old['phase'] != 'compiled' or not old['original_object_byte_equal'] or not old['default0_object_byte_equal']:
        raise RuntimeError('checked original Engram object lineage differs')
    if old['original_object_sha256'] != '235a321d32f24b5807d7dab2f92b81651b12203ea476a55bd5ff69536fc5dfb6':
        raise RuntimeError('qualified native Engram object differs')
    args = old['runs'][0]['command']
    if args[:4] != ['nice', '-n', '19', '/usr/bin/c++']:
        raise RuntimeError('exact original Engram compiler prefix differs')
    original = Path(args[args.index('-c') + 1])
    if sha(original) != PLAN['original_source_sha256']:
        raise RuntimeError('pinned original model decoder source differs')
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
    for mode, source, flags in [('original', original, []), ('default0', ROOT / 'ds41_engram.cpp', []),
                                ('enabled1', ROOT / 'ds41_engram.cpp', ['-DASTER_XQ_ENGRAM_FP8_SIMD=1'])]:
        directory = ROOT / mode
        # Test executables already occupy two names; object directories are explicit.
        directory = ROOT / ('object-' + mode)
        directory.mkdir(mode=0o700)
        target = directory / 'ds41_engram.cpp.o'
        command = [*clean, *flags, '-o', str(target), '-c', str(source)]
        run(command, 'model-' + mode + '-compile', env, 30)
        row = dict(mode=mode, command=command,
                   object=dict(path=str(target), bytes=target.stat().st_size, sha256=sha(target)))
        STATE['runs'].append(row)
        if mode in ['original', 'default0'] and row['object']['sha256'] != old['original_object_sha256']:
            raise RuntimeError('original/default0 model Engram object is not byte-identical')
        save()
    enabled = STATE['tests'][1]['result']
    old_total = sum(r['reference_seconds'] for r in enabled['rounds'])
    new_total = sum(r['candidate_seconds'] for r in enabled['rounds'])
    STATE.update(phase='compiled', original_object_byte_equal=True, default0_object_byte_equal=True,
                 bitwise_decoder_qualified=True, candidate_object=STATE['runs'][-1]['object'],
                 cpu_only_microbenchmark_ratio=old_total/new_total,
                 cpu_only_microbenchmark_reduction_percent=(1-new_total/old_total)*100,
                 native_linked=False, gpu_qualified=False, finished_unix=time.time())
except BaseException as error:
    STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
    raise
finally:
    save()
