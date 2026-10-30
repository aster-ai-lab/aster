"""Build only CPU host objects on the jump host; no CUDA compiler/runtime call."""
import hashlib
import json
import os
import shlex
import subprocess
import time
from pathlib import Path

ROOT=Path(__file__).resolve().parent
P=json.loads((ROOT/'cpu-plan.json').read_text())
STATE=dict(schema='aster-indexer-v40-cpu-preflight',unix=time.time(),phase='preflight',gpu_executed=False,cuda_kernel_compiled=False,performance_claimed=False)

def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save():
    p=ROOT/'cpu-result.tmp';p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(ROOT/'cpu-result.json')
def run(args,name,seconds):
    STATE['step']=name;save()
    with (ROOT/(name+'.stdout.log')).open('xb') as out,(ROOT/(name+'.stderr.log')).open('xb') as err:
        q=subprocess.run(args,cwd=ROOT,stdin=subprocess.DEVNULL,stdout=out,stderr=err,timeout=seconds)
    if q.returncode:raise RuntimeError(name+' exit '+str(q.returncode))
try:
    os.umask(0o077);os.environ['CUDA_VISIBLE_DEVICES']='';save()
    proof=json.loads((ROOT.parent/'resource-result.json').read_text())
    if os.getuid()!=10250 or ROOT.resolve()!=ROOT or ROOT.stat().st_uid!=10250 or proof['phase']!='passed' or time.time()+200>proof['leaseEndsUnix']:
        raise RuntimeError('exact bounded CPU preflight owner/resource differs')
    for row in P['sources']:
        p=ROOT/row['file']
        if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or sha(p)!=row['sha256']:raise RuntimeError('frozen indexer preflight source differs')
    parent=Path(P['parent'])
    lib=parent/'build/libds41_dense.a';source=parent/'source/tools/v41/engine/ds41_dense.cpp'
    if sha(lib)!=P['archive_sha256'] or sha(source)!=P['source_sha256']:raise RuntimeError('actual deployed dense parent changed')
    run(['/usr/bin/c++','-O2','-std=c++20','-Wall','-Wextra','-Werror',str(ROOT/'indexer_mask_policy_test.cpp'),'-o',str(ROOT/'indexer_mask_policy_test')],'policy-compile',15)
    run([str(ROOT/'indexer_mask_policy_test')],'policy-test',10)
    STATE['host_policy_and_reassociation_passed']=True;save()
    args=shlex.split(P['compile_command']);clean=[];i=0
    while i<len(args):
        if args[i] in ['-MT','-MF']:i+=2;continue
        if args[i]=='-MD':i+=1;continue
        clean.append(args[i]);i+=1
    original=list(clean)
    original[original.index('-o')+1]=str(ROOT/'ds41_dense.original.o')
    original[original.index('-c')+1]=str(source)
    run(original,'original-dense-control-compile',70)
    q=subprocess.run(['ar','p',str(lib),'ds41_dense.cpp.o'],capture_output=True,check=True,timeout=5)
    if len(q.stdout)>2*1024**2:raise RuntimeError('bounded original dense archive object exceeded')
    with (ROOT/'ds41_dense.archive-original.o').open('xb') as f:f.write(q.stdout)
    STATE.update(original_archive_member_sha256=hashlib.sha256(q.stdout).hexdigest(),recompiled_original_sha256=sha(ROOT/'ds41_dense.original.o'))
    STATE['original_dense_object_byte_equal']=STATE['original_archive_member_sha256']==STATE['recompiled_original_sha256'];save()
    candidate=list(clean)
    candidate[candidate.index('-o')+1]=str(ROOT/'ds41_dense.cpp.o')
    candidate[candidate.index('-c')+1]=str(ROOT/'ds41_dense.cpp')
    run(candidate,'candidate-dense-host-compile',70)
    STATE.update(candidate_host_compiled=True,candidate_host_object_sha256=sha(ROOT/'ds41_dense.cpp.o'))
    if not STATE['original_dense_object_byte_equal']:
        raise RuntimeError('recompiled original dense object differs; inspect compiler/path/flags before a model candidate link')
    STATE.update(phase='passed',finished_unix=time.time())
except BaseException as e:
    STATE.update(phase='needs_inspection',error=type(e).__name__+': '+str(e),finished_unix=time.time());raise
finally:save()
