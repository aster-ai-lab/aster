"""Controlled generator build and real CPU pool/metadata contracts; no CUDA or model execution."""
import hashlib
import json
import os
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PLAN = json.loads((ROOT / 'build-plan.json').read_text())
STATE = dict(unix=time.time(), phase='preflight', gpu_executed=False, native_linked=False,
             gpu_qualified=False, performance_claimed=False, runs=[], unit_tests=[])


def save():
    p = ROOT / 'build-result.tmp'; p.write_text(json.dumps(STATE, indent=2) + '\n'); p.replace(ROOT / 'build-result.json')


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()


def command(args, label, env, timeout=100):
    STATE.update(step=label); save()
    with (ROOT / (label + '.log')).open('xb') as log:
        p = subprocess.run(['nice','-n','19',*args], cwd=ROOT, env=env, stdout=log, stderr=log, timeout=timeout)
    if p.returncode:
        raise RuntimeError('controlled CPU command failed: ' + label)


try:
    os.umask(0o077); save()
    if os.getuid()!=10250 or ROOT.resolve()!=ROOT or ROOT.stat().st_uid!=10250:
        raise RuntimeError('exact CPU compilation artifact owner differs')
    old=Path(PLAN['old_cpu_root']);proof=json.loads((old/'build-result.json').read_text())
    if proof['phase']!='compiled' or not proof['control_object_byte_equal']:
        raise RuntimeError('exact c96 generator CPU provenance differs')
    original=old/'ds41_generate.cpp'
    if sha(original)!=PLAN['original_source_sha256']:
        raise RuntimeError('exact parent generator source differs')
    for row in PLAN['inputs']:
        p=ROOT/row['file']
        if p.is_symlink() or p.stat().st_size!=row['bytes'] or sha(p)!=row['sha256']:
            raise RuntimeError('frozen CPU input differs: '+p.name)
    args=list(proof['runs'][-1]['command'])
    if args[:3]!=['nice','-n','19']:
        raise RuntimeError('exact parent CPU scheduling prefix differs')
    args=args[3:];clean=[];i=0
    while i<len(args):
        if args[i] in ['-o','-c','-MT','-MF']:i+=2;continue
        if args[i]=='-MD':i+=1;continue
        clean.append(args[i]);i+=1
    if clean[0]!='/usr/bin/c++':raise RuntimeError('exact controlled host compiler differs')
    env={k:v for k,v in os.environ.items() if not k.startswith(('STRATA_','ASTER_XQ_'))}
    env.update(CUDA_VISIBLE_DEVICES='',OMP_NUM_THREADS='1')
    for name,source in [('control',original),('candidate',ROOT/'ds41_generate.cpp')]:
        STATE.update(phase='compiling',step=name);save();target=ROOT/('generator-'+name+'.o')
        args=[*clean,'-o',str(target),'-c',str(source)]
        command(args,name+'-compile',env)
        row=dict(name=name,command=['nice','-n','19',*args],object=dict(path=str(target),bytes=target.stat().st_size,sha256=sha(target)))
        STATE['runs'].append(row)
        if name=='control':
            if row['object']['sha256']!=proof['candidate_object']['sha256']:
                raise RuntimeError('exact c96 parent generator object did not reproduce')
            STATE['control_object_byte_equal']=True
        save()
    include=['-I'+str(ROOT),'-I'+PLAN['original_engine_headers']]
    for mode,flags in [('release',['-O2']),('asan_ubsan',['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-fno-sanitize-recover=all'])]:
        target=ROOT/('test-pool-seed-'+mode)
        args=['/usr/bin/c++','-std=c++20','-Wall','-Wextra','-Werror',*flags,*include,str(ROOT/'test_pool_seed.cpp'),str(ROOT/'ds41_conv_pool.cpp'),'-pthread','-o',str(target)]
        command(args,'metadata-'+mode+'-compile',env)
        test_env=dict(env,ASAN_OPTIONS='detect_leaks=1:abort_on_error=1',UBSAN_OPTIONS='halt_on_error=1')
        STATE.update(step='metadata-'+mode+'-run');save()
        p=subprocess.run(['nice','-n','19',str(target)],cwd=ROOT,env=test_env,capture_output=True,text=True,timeout=40)
        (ROOT/('metadata-'+mode+'-run.log')).write_text(p.stdout+p.stderr)
        words=p.stdout.strip().split()
        if p.returncode or len(words)!=4 or words[0]!='pool_seed:' or words[2:]!=['contracts','passed']:
            raise RuntimeError('real CPU pool/seed contracts failed: '+mode)
        STATE['unit_tests'].append(dict(mode=mode,contracts=int(words[1]),exit=p.returncode,stdout=p.stdout))
        save()
    target=ROOT/'test-original-pool'
    args=['/usr/bin/c++','-std=c++20','-O2',*include,str(ROOT/'test_ds41_conv_pool.cpp'),str(ROOT/'ds41_conv_pool.cpp'),'-pthread','-o',str(target)]
    command(args,'upstream-pool-compile',env)
    p=subprocess.run(['nice','-n','19',str(target)],cwd=ROOT,env=env,capture_output=True,text=True,timeout=40)
    (ROOT/'upstream-pool-run.log').write_text(p.stdout+p.stderr)
    if p.returncode or not p.stdout.strip().endswith('test_ds41_conv_pool: all passed'):
        raise RuntimeError('unchanged upstream CPU pool suite failed')
    STATE['upstream_pool_suite_passed']=True
    STATE.update(phase='compiled',candidate_object=STATE['runs'][-1]['object'],finished_unix=time.time());save()
except BaseException as error:
    STATE.update(phase='failed',error=type(error).__name__+': '+str(error),finished_unix=time.time());save();raise
