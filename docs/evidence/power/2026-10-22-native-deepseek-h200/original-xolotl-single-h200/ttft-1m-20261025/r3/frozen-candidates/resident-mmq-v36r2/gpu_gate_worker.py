"""Real-weight F32 gates and paired operator timings on the one idle test GPU."""
import hashlib
import json
import os
import re
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
P = json.loads((ROOT/'gpu-plan.json').read_text())
STATE = dict(schema='aster-xolotl-resident-mmq-v35-gpu-gate',unix=time.time(),phase='preflight',runs=[],model_api_speedup_claimed=False)

def save():
    p = ROOT/'gpu-result.tmp'
    p.write_text(json.dumps(STATE,indent=2)+'\n')
    p.replace(ROOT/'gpu-result.json')

def run(name, args, env, budget):
    STATE['phase'] = name
    save()
    with (ROOT/(name+'.stdout.log')).open('xb') as out, (ROOT/(name+'.stderr.log')).open('xb') as err:
        p = subprocess.run(args,cwd=ROOT,env=env,stdin=subprocess.DEVNULL,stdout=out,stderr=err,timeout=budget)
    stdout = (ROOT/(name+'.stdout.log')).read_text()
    stderr = (ROOT/(name+'.stderr.log')).read_text()
    if len(stdout)>262144 or len(stderr)>262144:
        raise RuntimeError('bounded numerical gate log exceeded')
    row = dict(name=name,returncode=p.returncode,stdout=stdout,stderr=stderr)
    STATE['runs'].append(row)
    save()
    if p.returncode:
        raise RuntimeError(name+' exit '+str(p.returncode))
    return row

try:
    os.umask(0o077)
    save()
    proof = json.loads((ROOT.parent/'resource-result.json').read_text())
    if os.getuid()!=10250 or ROOT.resolve()!=ROOT or proof['phase']!='passed' or proof['gpus'][0]['uuid']!=P['gpu_uuid'] or time.time()+480>proof['leaseEndsUnix']:
        raise RuntimeError('exact GPU gate resource identity/budget differs')
    gpu = subprocess.run(['nvidia-smi','--query-gpu=uuid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
    apps = subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
    if gpu!=P['gpu_uuid'] or apps:
        raise RuntimeError('numerical gate requires its independent idle GPU')
    built = json.loads((ROOT/'result.json').read_text())
    if built['phase']!='built' or not built['resident_stage_policy_tests_passed'] or not built['cache_policy_tests_passed']:
        raise RuntimeError('compiled gates/policies must pass first')
    for row in [built['binary'],built['real_weight_gate_binary'],P['slot_gate']]:
        path = Path(row['path'])
        if path.is_symlink() or path.stat().st_uid!=10250 or hashlib.sha256(path.read_bytes()).hexdigest()!=row['sha256']:
            raise RuntimeError('exact compiled numerical gate binary differs')
    env = {k:v for k,v in os.environ.items() if not k.startswith(('STRATA_','ASTER_XQ_','DS41_','MIMO_'))}
    env.update(CUDA_VISIBLE_DEVICES='0',STRATA_NO_LARGEPAGES='1',GGML_NO_BACKTRACE='1',OMP_NUM_THREADS='16',OPENBLAS_NUM_THREADS='1',MIMO_CHUNK_PROF='1')
    STATE.update(gpu_uuid=gpu,lease_ends_unix=proof['leaseEndsUnix'],binary_sha256=built['binary']['sha256'])
    slot = run('sized-slot-byte-gate',[P['slot_gate']['path']],env,15)
    if not slot['stdout'].startswith('PASS mixed cache: 3 shrink/grow cycles, '):
        raise RuntimeError('exact slot byte gate missing')
    for flag in [0,1]:
        e = dict(env,ASTER_XQ_RESIDENT_MMQ=str(flag),MIMO_CHUNK_STAGE_MIB='32')
        row = run('real-f32-reference-'+str(flag),[built['real_weight_gate_binary']['path'],P['weights'],'--layers','0,39','--tokens','2','--gpu','--chunk','8','--chunk-mmq'],e,150)
        if '-> PASS' not in row['stdout'] or 'non-finite' not in row['stdout'] or 'FAIL' in row['stdout']:
            raise RuntimeError('real-weight dequant F32 gate incomplete')
        waves = [int(x) for x in re.findall(r'resident MMQ stage: (\d+) waves,',row['stderr'])]
        if flag and sum(waves)==0:
            raise RuntimeError('real-weight gate did not exercise resident packing')
        row['resident_packing_waves'] = sum(waves)
        save()
    for flag in [0,1]:
        e = dict(env,ASTER_XQ_RESIDENT_MMQ=str(flag),MIMO_CHUNK_STAGE_MIB='512')
        row = run('paired-operator-timing-'+str(flag),[built['real_weight_gate_binary']['path'],P['weights'],'--layers','0,39','--tokens','0','--gpu','--chunk','512','--chunk-mmq','--no-ref'],e,90)
        row['layer_timings'] = [dict(layer=int(a),chunk_tokens=int(b),milliseconds=float(c)) for a,b,c in re.findall(r'time L(\d+) chunk (\d+) tokens \(\d+ remote entries\): ([\d.]+) ms',row['stdout'])]
        if len(row['layer_timings'])!=6:
            raise RuntimeError('paired 2-layer three-repetition operator timing incomplete')
        save()
    STATE.update(phase='passed',finished_unix=time.time(),real_weight_f32_reference_passed=True,paired_operator_timing_completed=True)
except BaseException as error:
    STATE.update(phase='failed',error=type(error).__name__+': '+str(error),finished_unix=time.time())
    raise
finally:
    save()
