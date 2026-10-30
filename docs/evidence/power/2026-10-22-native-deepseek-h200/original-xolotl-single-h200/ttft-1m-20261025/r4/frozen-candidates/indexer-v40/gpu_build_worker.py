"""GPU-free controlled link of exact-mask CUDA/dense candidates and gates."""
import hashlib
import json
import os
import shutil
import subprocess
import time
from pathlib import Path

ROOT=Path(__file__).resolve().parent
P=json.loads((ROOT/'gpu-build-plan.json').read_text())
STATE=dict(schema='aster-indexer-mask-v40-build',unix=time.time(),phase='preflight',gpu_executed=False,performance_claimed=False)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save():
    p=ROOT/'gpu-build-result.tmp';p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(ROOT/'gpu-build-result.json')
def run(args,name,seconds,cwd=None):
    STATE['step']=name;save()
    with (ROOT/(name+'.stdout.log')).open('xb') as out,(ROOT/(name+'.stderr.log')).open('xb') as err:
        q=subprocess.run(args,cwd=cwd or ROOT,stdin=subprocess.DEVNULL,stdout=out,stderr=err,timeout=seconds)
    if q.returncode:raise RuntimeError(name+' exit '+str(q.returncode))
try:
    os.umask(0o077);os.environ['CUDA_VISIBLE_DEVICES']='';save()
    proof=json.loads((ROOT.parent/'resource-result.json').read_text())
    if os.getuid()!=10250 or ROOT.resolve()!=ROOT or ROOT.stat().st_uid!=10250 or proof['phase']!='passed' or time.time()+600>proof['leaseEndsUnix']:raise RuntimeError('controlled CUDA build resource/lease differs')
    for row in P['sources']:
        p=ROOT/row['file']
        if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or sha(p)!=row['sha256']:raise RuntimeError('frozen CUDA/dense build input differs')
    parent=Path(P['parent']);cpu=Path(P['cpu_root'])
    q=json.loads((cpu/'cpu-result.json').read_text())
    if q['phase']!='passed' or not q['original_dense_object_byte_equal'] or sha(cpu/'ds41_dense.cpp.o')!=q['candidate_host_object_sha256']:raise RuntimeError('exact byte-equal original dense CPU preflight required')
    if sha(parent/'build/libds41_dense.a')!=P['dense_archive_sha256'] or sha(parent/'source/third_party/llama.cpp/ggml/src/ggml-cuda/lightning-indexer.cu')!=P['cuda_original_source_sha256']:raise RuntimeError('qualified parent archive/source differs')
    link=list(P['v35_control_link'])
    control=list(link);control[control.index('-o')+1]=str(ROOT/'ds41_generate.v35_control')
    run(control,'exact-v35-control-relink',35,parent/'build')
    if sha(ROOT/'ds41_generate.v35_control')!=P['v35_binary_sha256']:raise RuntimeError('controlled relink does not reproduce qualified V35')
    STATE['v35_control_binary_sha256']=sha(ROOT/'ds41_generate.v35_control');save()
    run(P['cuda_compile'],'exact-mask-cuda-compile',210)
    run(P['probe_compile'],'exact-mask-probe-compile',90)
    run(P['graph_compile'],'actual-graph-driver-compile',25)
    shutil.copyfile(cpu/'ds41_dense.cpp.o',ROOT/'ds41_dense.cpp.o')
    shutil.copyfile(parent/'build/libds41_dense.a',ROOT/'libds41_dense.a')
    run(['ar','r',str(ROOT/'libds41_dense.a'),str(ROOT/'ds41_dense.cpp.o')],'dense-archive-replace',8)
    candidate=list(link)
    candidate[candidate.index('libds41_dense.a')]=str(ROOT/'libds41_dense.a')
    candidate.insert(candidate.index('ggml/src/ggml-cuda/libggml-cuda.a'),str(ROOT/'lightning-indexer.cu.o'))
    candidate[candidate.index('-o')+1]=str(ROOT/'ds41_generate')
    run(candidate,'candidate-model-link',35,parent/'build')
    for name,command in [('graph-gate-original',link),('graph-gate-candidate',candidate),('helper-probe',link)]:
        args=list(command)
        host=P['generate_host_object']
        if args.count(host)!=1:raise RuntimeError('exact model host object substitution differs')
        args[args.index(host)]=str(ROOT/('indexer_mask_probe.cu.o' if name=='helper-probe' else 'indexer_graph_gate.cpp.o'))
        args[args.index('-o')+1]=str(ROOT/name)
        run(args,name+'-link',35,parent/'build')
    STATE.update(phase='built',gpu_quality_passed=False,adopted=False,
                 candidate=dict(path=str(ROOT/'ds41_generate'),sha256=sha(ROOT/'ds41_generate'),bytes=(ROOT/'ds41_generate').stat().st_size),
                 gates=[dict(name=n,path=str(ROOT/n),sha256=sha(ROOT/n)) for n in ['graph-gate-original','graph-gate-candidate','helper-probe']],
                 cuda_object_sha256=sha(ROOT/'lightning-indexer.cu.o'),dense_object_sha256=sha(ROOT/'ds41_dense.cpp.o'),finished_unix=time.time())
except BaseException as error:
    STATE.update(phase='failed',error=type(error).__name__+': '+str(error),finished_unix=time.time());raise
finally:save()
