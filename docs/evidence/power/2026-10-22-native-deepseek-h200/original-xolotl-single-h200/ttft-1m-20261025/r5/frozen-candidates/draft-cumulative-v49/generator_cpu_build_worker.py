"""Compile the actual generator on the jump CPU with exact CUDA host headers; execute no GPU code."""
import hashlib
import json
import os
import shlex
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PLAN = json.loads((ROOT / 'generator-build-plan.json').read_text())
STATE = dict(unix=time.time(), phase='preflight', gpu_executed=False, runs=[])


def save():
    temporary = ROOT / 'generator-cpu-build-result.tmp'
    temporary.write_text(json.dumps(STATE, indent=2) + '\n')
    temporary.replace(ROOT / 'generator-cpu-build-result.json')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


try:
    os.umask(0o077)
    save()
    if ROOT.resolve() != ROOT or ROOT.stat().st_uid != 10250 or os.getuid() != 10250:
        raise RuntimeError('exact CPU generator build owner differs')
    headers = json.loads((ROOT / 'cuda-header-result.json').read_text())
    for row in headers['headers']:
        path = ROOT / 'cuda-include' / row['file']
        if path.is_symlink() or path.stat().st_size != row['bytes'] or digest(path) != row['sha256']:
            raise RuntimeError('exact staged CUDA header differs')
    for row in PLAN['inputs']:
        path = Path(row['path'])
        if path.is_symlink() or path.stat().st_uid != 10250 or path.stat().st_size != row['bytes'] or digest(path) != row['sha256']:
            raise RuntimeError('frozen generator CPU source differs: ' + row['path'])
    original = Path(PLAN['original_source'])
    if digest(original) != PLAN['original_source_sha256']:
        raise RuntimeError('original source identity differs')
    command = shlex.split(PLAN['compile_command'])
    clean = []
    index = 0
    while index < len(command):
        if command[index] in ['-MT', '-MF', '-o', '-c']:
            index += 2
            continue
        if command[index] == '-MD':
            index += 1
            continue
        clean.append(command[index])
        index += 1
    if clean[0] != '/usr/bin/c++' or clean.count('-isystem') != 1:
        raise RuntimeError('exact controlled C++ compiler invocation differs')
    include_index = clean.index('-isystem') + 1
    if clean[include_index] != headers['original_include_root']:
        raise RuntimeError('exact CUDA host include root differs')
    clean[include_index] = headers['header_root']
    # Header __FILE__ values retain the original spelling. No arithmetic flags change.
    clean += ['-fmacro-prefix-map=' + headers['header_root'] + '=' + headers['original_include_root']]
    env = {key: value for key, value in os.environ.items() if not key.startswith(('STRATA_', 'ASTER_XQ_'))}
    env.update(CUDA_VISIBLE_DEVICES='', OMP_NUM_THREADS='1')
    for name, source in [('original', original), ('candidate', ROOT / 'ds41_generate.cpp')]:
        STATE.update(phase='compiling', step=name)
        save()
        output = ROOT / ('generator-' + name + '.o')
        args = clean + ['-o', str(output), '-c', str(source)]
        with (ROOT / ('generator-' + name + '-compile.log')).open('xb') as log:
            process = subprocess.run(args, cwd=ROOT, env=env, stdout=log, stderr=log, timeout=95)
        row = dict(name=name, returncode=process.returncode, command=args)
        STATE['runs'].append(row)
        save()
        if process.returncode:
            raise RuntimeError('controlled C++ generator compile failed: ' + name)
        row['object'] = dict(path=str(output), bytes=output.stat().st_size, sha256=digest(output))
        if name == 'original':
            if row['object']['sha256'] != PLAN['original_object_sha256']:
                raise RuntimeError('recompiled original generator object is not byte-identical')
            STATE['original_generator_object_byte_equal'] = True
        save()
    STATE.update(phase='compiled', generator_compiled=True, candidate_object=STATE['runs'][-1]['object'],
                 native_binary_linked=False, gpu_qualified=False, performance_claimed=False)
except BaseException as error:
    STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error))
    raise
finally:
    STATE['finished_unix'] = time.time()
    save()
