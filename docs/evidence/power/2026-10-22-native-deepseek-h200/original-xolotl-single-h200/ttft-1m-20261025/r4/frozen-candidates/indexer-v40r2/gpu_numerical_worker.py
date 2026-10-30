"""Exact full-score identity and conservative CUDA fallback gates, not model speed."""
import hashlib
import json
import os
import struct
import subprocess
import time
from pathlib import Path

ROOT=Path(__file__).resolve().parent
P=json.loads((ROOT/'gpu-numerical-plan.json').read_text())
STATE=dict(schema='aster-indexer-v40-gpu-numerical',unix=time.time(),phase='preflight',runs=[],model_quality_claimed=False,performance_claimed=False)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save():
    p=ROOT/'gpu-numerical-result.tmp';p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(ROOT/'gpu-numerical-result.json')
def run(name,args,env,seconds):
    STATE['phase']=name;save()
    with (ROOT/(name+'.stdout.log')).open('xb') as out,(ROOT/(name+'.stderr.log')).open('xb') as err:
        t=time.monotonic();q=subprocess.run(args,cwd=ROOT,env=env,stdin=subprocess.DEVNULL,stdout=out,stderr=err,timeout=seconds)
    a=(ROOT/(name+'.stdout.log')).read_text();b=(ROOT/(name+'.stderr.log')).read_text()
    if len(a)+len(b)>262144:raise RuntimeError('bounded numerical log exceeded')
    row=dict(name=name,returncode=q.returncode,seconds=time.monotonic()-t,stdout=a,stderr=b);STATE['runs'].append(row);save()
    if q.returncode:raise RuntimeError(name+' exit '+str(q.returncode))
    return row
try:
    os.umask(0o077);save()
    proof=json.loads((ROOT.parent/'resource-result.json').read_text())
    if os.getuid()!=10250 or ROOT.resolve()!=ROOT or proof['phase']!='passed' or proof['gpus'][0]['uuid']!=P['gpu_uuid'] or time.time()+400>proof['leaseEndsUnix']:raise RuntimeError('exact numerical GPU lease differs')
    gpu=subprocess.run(['nvidia-smi','--query-gpu=uuid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
    apps=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
    if gpu!=P['gpu_uuid'] or apps:raise RuntimeError('numerical gates require their single idle GPU')
    built=json.loads((ROOT/'gpu-build-result.json').read_text())
    if built['phase']!='built':raise RuntimeError('controlled CUDA build incomplete')
    gates={x['name']:x for x in built['gates']}
    for row in gates.values():
        p=Path(row['path'])
        if p.is_symlink() or p.stat().st_uid!=10250 or sha(p)!=row['sha256']:raise RuntimeError('exact numerical binary differs')
    env={k:v for k,v in os.environ.items() if not k.startswith(('STRATA_','ASTER_XQ_','DS41_','MIMO_'))}
    env.update(CUDA_VISIBLE_DEVICES='0',GGML_NO_BACKTRACE='1',OMP_NUM_THREADS='16',OPENBLAS_NUM_THREADS='1')
    row=run('helper-eligibility-write-fallback',[gates['helper-probe']['path']],env,60)
    if row['stdout'].strip()!='PASS exact masked-block CUDA eligibility/write/fallback: 144 cases':raise RuntimeError('complete144 CUDA eligibility cases required')
    reference=None;results=[]
    for name,binary,flag,mask_first in [('archive-reference','graph-gate-original','0','0'),('new-cuda-off','graph-gate-candidate','0','0'),('graph-mask-only','graph-gate-candidate','0','1'),('graph-mask-and-skip','graph-gate-candidate','1','1')]:
        dest=ROOT/(name+'.scores.bin')
        row=run(name,[gates[binary]['path'],str(dest),mask_first],dict(env,ASTER_XQ_INDEXER_MASK_SKIP=flag),75)
        if 'PASS actual CUDA indexer score cases: 90' not in row['stdout']:raise RuntimeError('complete90 real CUDA graphs required')
        if dest.is_symlink() or dest.stat().st_size>2*1024**2:raise RuntimeError('bounded score file exceeded')
        raw=dest.read_bytes();offset=0;cases=0;values=0
        while offset<len(raw):
            if offset+4>len(raw):raise RuntimeError('truncated score header')
            n=struct.unpack_from('<I',raw,offset)[0];offset+=4
            if n!=1542 or offset+4*n>len(raw):raise RuntimeError('exact graph score shape differs')
            offset+=4*n;cases+=1;values+=n
        if cases!=90:raise RuntimeError('score case count differs')
        if reference is None:reference=raw
        equal=raw==reference
        results.append(dict(profile=name,mask_first=mask_first,skip_flag=flag,cases=cases,score_values=values,sha256=hashlib.sha256(raw).hexdigest(),byte_equal_to_actual_archive=equal))
        STATE['score_comparisons']=results;save()
        if not equal:raise RuntimeError(name+' full-score byte identity differs; no model promotion')
    STATE.update(phase='passed',gpu_uuid=gpu,helper_cases=144,actual_graph_cases=360,score_values_per_profile=138780,full_score_byte_identity_passed=True,finished_unix=time.time())
except BaseException as error:
    STATE.update(phase='failed',error=type(error).__name__+': '+str(error),finished_unix=time.time());raise
finally:save()
