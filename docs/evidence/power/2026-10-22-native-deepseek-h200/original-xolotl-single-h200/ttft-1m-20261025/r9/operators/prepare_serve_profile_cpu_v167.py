"""Derive finite CPU lifecycle tests and exact reproduced generator compilation."""
import ast
import hashlib
import json
from xolotl_trial_common import BASE

worker=(BASE/'prefill_phase_cpu_worker_v134.frozen.py').read_text()
worker=worker.replace('4894588c1cd86d1207045f57c358dc5e23c1b61302d623a73ccdc82a6454dd22',
    '8ab829c4157968935bb4f95b31b73ebd1b60c6fb3fa6e6db144c30fc496c3715')
anchor="    for label, source in [('control', original), ('candidate', ROOT / 'ds41_generate.cpp')]:"
if worker.count(anchor)!=1:raise RuntimeError('unique CPU profile generator loop differs')
test='''    STATE['tests'] = []
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
'''
worker=worker.replace(anchor,test+anchor)
manager=(BASE/'manage_prefill_phase_cpu_v134.py').read_text()
manager=manager.replace('xolotl-prefill-phase-cpu-v134','xolotl-serve-profile-cpu-v167').replace('cpu-prefill-phase-v134','cpu-serve-profile-v167')
manager=manager.replace("BASE / 'prefill-phase-trace-v133r1'", "BASE / 'serve-decode-profile-v166'")
manager=manager.replace("BASE / 'xolotl-pool-seed-cpu-v105r2-intent.json'", "BASE / 'xolotl-prefill-phase-cpu-v134-intent.json'")
manager=manager.replace('prefill_phase_cpu_worker_v134.frozen.py','serve_decode_profile_cpu_worker_v167.frozen.py')
anchor="plan = dict(parent_cpu_root=parent['root'], parent_source_sha256=manifest['parent_source_sha256'], inputs=inputs)"
if manager.count(anchor)!=1:raise RuntimeError('unique profile CPU provenance plan differs')
manager=manager.replace(anchor,"link = json.loads((BASE / 'xolotl-native-phase-simd-v138-plan.json').read_text())\n    moe = [p for p in link['control_link'] if p.endswith('/build-resident-mmq-v35/ds41_moe.cpp.o')]\n    if len(moe) != 1:raise RuntimeError('exact original MoE object provenance differs')\n    plan = dict(parent_cpu_root=parent['root'], parent_source_sha256=manifest['parent_source_sha256'], inputs=inputs, moe_object=moe[0])")
rows=[]
for name,text in [('serve_decode_profile_cpu_worker_v167.frozen.py',worker),('manage_serve_profile_cpu_v167.py',manager)]:
    raw=text.encode();ast.parse(raw,filename=name)
    with (BASE/name).open('xb') as output:output.write(raw)
    rows.append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
(BASE/'xolotl-serve-profile-cpu-v167-prepared.json').write_text(json.dumps(dict(files=rows,gpu_executed=False,model_math_changed=False),indent=2)+'\n')
print(json.dumps(dict(phase='prepared',files=len(rows),gpu_executed=False)))
