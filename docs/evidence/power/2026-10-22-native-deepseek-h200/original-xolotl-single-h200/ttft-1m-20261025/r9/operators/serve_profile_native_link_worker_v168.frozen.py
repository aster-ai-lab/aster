"""Reproduce native99f3 and replace only its GEN host object with default-off profiling hooks."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT=Path(__file__).resolve().parent
P=json.loads((ROOT/'native-link-plan.json').read_text())
STATE=dict(unix=time.time(),phase='preflight',gpu_math_executed=False,model_math_changed=False,
    performance_claimed=False,gpu_qualified=False)
def save():
    p=ROOT/'native-link-result.tmp';p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(ROOT/'native-link-result.json')
def sha(path):
    h=hashlib.sha256()
    with path.open('rb') as source:
        while data:=source.read(4*1024**2):h.update(data)
    return h.hexdigest()
try:
    os.umask(0o077);os.environ['CUDA_VISIBLE_DEVICES']='';save()
    if os.getuid()!=10250 or ROOT.resolve()!=ROOT or ROOT.stat().st_uid!=10250:raise RuntimeError('exact profiler native build owner differs')
    proof=json.loads((ROOT.parent/'resource-result.json').read_text())
    old=json.loads((Path(P['prior_service_root'])/'service-longctx-v1-result.json').read_text())
    if proof['phase']!='passed' or proof['gpus'][0]['uuid']!=P['gpu_uuid'] or P['gpu_uuid']==P['formal_gpu_uuid'] or time.time()+220>proof['leaseEndsUnix'] or old['phase']!='stopped' or not old.get('api_reaped'):
        raise RuntimeError('finite independent idle profiling link required')
    q=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5)
    if q.stdout.strip():raise RuntimeError('exact optimization GPU must be idle before linking')
    cpu=json.loads((Path(P['profile_cpu_root'])/'build-result.json').read_text())
    if cpu['phase']!='compiled' or not cpu['control_object_byte_equal'] or not cpu['original_moe_profiler_CUDA_hooks_present'] or [x['result']['contracts'] for x in cpu['tests']]!=[20015,20015]:
        raise RuntimeError('exact controlled profiler CPU/sanitizer proof required')
    for row in P['inputs']:
        p=Path(row['path'])
        if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or sha(p)!=row['sha256']:raise RuntimeError('exact profiling native input differs: '+p.name)
    command=list(P['control_link'])
    if command.count(P['original_generator'])!=1 or command.count('-o')!=1:raise RuntimeError('exact controlled native substitution differs')
    STATE['links']=[]
    for label in ['control','profile']:
        target=ROOT/('ds41_generate.'+label)
        args=list(command);args[args.index('-o')+1]=str(target)
        if label=='profile':args[args.index(P['original_generator'])]=cpu['candidate_object']['path']
        STATE.update(phase='linking',current=label);save()
        with (ROOT/(label+'-link.log')).open('xb') as output:
            q=subprocess.run(args,cwd=P['link_cwd'],stdout=output,stderr=output,timeout=50)
        if q.returncode:raise RuntimeError('controlled profiler link failed: '+label)
        row=dict(profile=label,path=str(target),bytes=target.stat().st_size,sha256=sha(target))
        if label=='control':
            if row['sha256']!=P['control_binary_sha256']:raise RuntimeError('exact native99f3 did not reproduce byte-for-byte')
            STATE['control_binary_byte_equal']=True
        STATE['links'].append(row);save()
    env={key:os.environ[key] for key in ['PATH','LD_LIBRARY_PATH'] if key in os.environ}
    env['CUDA_VISIBLE_DEVICES']=''
    rejected=[]
    for value in ['', '2','-1','true','01',' 1','0 ','NaN']:
        q=subprocess.run([str(ROOT/'ds41_generate.profile'),'--help'],env=dict(env,ASTER_XQ_SERVE_PROFILE=value),capture_output=True,text=True,timeout=5)
        if q.returncode!=2 or q.stderr.strip()!='ASTER_XQ_SERVE_PROFILE must be 0 or 1':raise RuntimeError('invalid serve profiler flag not refused before backend startup')
        rejected.append(value)
    STATE.update(phase='linked',binary=STATE['links'][-1],only_generator_replaced=True,
        all_original_GPU_archives_unchanged=True,invalid_modes_refused=len(rejected),finished_unix=time.time());save()
except BaseException as error:
    STATE.update(phase='failed',error=type(error).__name__+': '+str(error)[:2000],finished_unix=time.time());save();raise
