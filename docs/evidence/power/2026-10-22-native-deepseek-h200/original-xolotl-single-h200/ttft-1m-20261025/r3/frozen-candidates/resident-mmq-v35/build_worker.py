"""GPU-free checked-cache compile; a successful build is not GPU qualification."""
import hashlib, json, os, shlex, subprocess, time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
P = json.loads((ROOT / 'plan.json').read_text())
STATE = dict(unix=time.time(), phase='preflight', gpu_tests_claimed=False, promoted=False)

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def save():
    temp = ROOT / 'result.tmp'
    temp.write_text(json.dumps(STATE, indent=2) + '\n')
    temp.replace(ROOT / 'result.json')

def run(args, name, seconds, cwd):
    STATE['step'] = name
    save()
    with (ROOT / (name + '.log')).open('xb') as log:
        result = subprocess.run(args, cwd=cwd, stdout=log, stderr=log, timeout=seconds)
    if result.returncode:
        raise RuntimeError(name + ' exit ' + str(result.returncode))

try:
    os.umask(0o077)
    os.environ['CUDA_VISIBLE_DEVICES'] = ''
    save()
    proof = json.loads((ROOT.parent / 'resource-result.json').read_text())
    if os.getuid() != 10250 or ROOT.resolve() != ROOT or ROOT.stat().st_uid != 10250:
        raise RuntimeError('GPU-free build owner differs')
    if proof['phase'] != 'passed' or time.time() + 240 > proof['leaseEndsUnix']:
        raise RuntimeError('bounded build lease insufficient')
    parent = Path(P['parent'])
    if sha(parent / 'source/tools/v41/engine/ds41_moe.cpp') != P['source_sha256']:
        raise RuntimeError('qualified MoE source differs')
    baseline = Path(P['v15_build_root'])
    if sha(baseline / 'ds41_generate') != P['v15_binary_sha256']:
        raise RuntimeError('qualified V15 candidate differs')
    for row in P['sources']:
        path = ROOT / row['file']
        if path.is_symlink() or path.stat().st_uid != 10250 or path.stat().st_size != row['bytes'] or sha(path) != row['sha256']:
            raise RuntimeError('frozen cache build input differs')
    command = P['link_command']
    if not command.startswith(': && ') or not command.endswith(' && :'):
        raise RuntimeError('exact link wrapper differs')
    link = shlex.split(command[5:-5])
    host_object = 'CMakeFiles/ds41_generate.dir/tools/v41/engine/ds41_generate.cpp.o'
    if link.count(host_object) != 1 or link.count('libds41_moe_cuda.a') != 1 or link.count('-o') != 1:
        raise RuntimeError('exact controlled link input differs')
    link[link.index(host_object)] = str(baseline / 'ds41_generate.cpp.o')
    control = list(link)
    control[control.index('-o') + 1] = str(ROOT / 'ds41_generate.control')
    run(control, 'v15-control-relink', 30, parent / 'build')
    if sha(ROOT / 'ds41_generate.control') != P['v15_binary_sha256']:
        raise RuntimeError('control relink does not reproduce V15')
    STATE['control_binary_sha256'] = sha(ROOT / 'ds41_generate.control')
    save()
    args = shlex.split(P['compile_command'])
    clean = []
    index = 0
    while index < len(args):
        if args[index] in ['-MT', '-MF']:
            index += 2
            continue
        if args[index] == '-MD':
            index += 1
            continue
        clean.append(args[index])
        index += 1
    clean[clean.index('-o') + 1] = str(ROOT / 'ds41_moe.cpp.o')
    clean[clean.index('-c') + 1] = str(ROOT / 'ds41_moe.cpp')
    run(clean, 'checked-moe-compile', 90, ROOT)
    run(['/usr/bin/c++', '-O2', '-std=c++20', '-Wall', '-Wextra', '-Werror', str(ROOT / 'cache_policy_test.cpp'), '-o', str(ROOT / 'cache_policy_test')],
        'cache-policy-compile', 15, ROOT)
    run([str(ROOT / 'cache_policy_test')], 'cache-policy-test', 10, ROOT)
    run(['/usr/bin/c++', '-O2', '-std=c++20', '-Wall', '-Wextra', '-Werror', str(ROOT / 'resident_stage_policy_test.cpp'), '-o', str(ROOT / 'resident_stage_policy_test')], 'resident-stage-policy-compile', 15, ROOT)
    run([str(ROOT / 'resident_stage_policy_test')], 'resident-stage-policy-test', 10, ROOT)
    candidate = list(link)
    # This object resolves every MoE definition before the unchanged archive;
    # other archive members and every GPU kernel keep their original inputs.
    candidate.insert(candidate.index('libds41_moe_cuda.a'), str(ROOT / 'ds41_moe.cpp.o'))
    candidate[candidate.index('-o') + 1] = str(ROOT / 'ds41_generate')
    run(candidate, 'checked-candidate-link', 30, parent / 'build')
    gate = list(candidate)
    gate[gate.index(str(baseline / 'ds41_generate.cpp.o'))] = str(ROOT / 'test_resident_mmq.cpp')
    gate[gate.index('-o') + 1] = str(ROOT / 'test_resident_mmq')
    gate[1:1] = ['-std=c++20', '-I'+str(parent/'source/include'), '-I'+str(parent/'source/tools/v41/engine'), '-I'+str(parent/'source/third_party/llama.cpp/ggml/include')]
    run(gate, 'real-weight-gate-compile', 45, parent / 'build')
    STATE['real_weight_gate_binary'] = dict(path=str(ROOT/'test_resident_mmq'), sha256=sha(ROOT/'test_resident_mmq'))
    STATE['resident_stage_policy_tests_passed'] = True
    STATE.update(phase='built', cache_policy_tests_passed=True,
                 binary=dict(path=str(ROOT / 'ds41_generate'), bytes=(ROOT / 'ds41_generate').stat().st_size,
                             sha256=sha(ROOT / 'ds41_generate')))
except BaseException as error:
    STATE.update(phase='failed', error=type(error).__name__ + ': ' + str(error))
    raise
finally:
    STATE['finished_unix'] = time.time()
    save()
