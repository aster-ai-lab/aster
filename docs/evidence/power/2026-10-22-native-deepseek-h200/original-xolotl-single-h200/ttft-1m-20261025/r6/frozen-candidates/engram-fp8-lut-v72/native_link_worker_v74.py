"""Reproduce native34f, then replace only the verified Engram archive member."""
import hashlib
import json
import os
import shutil
import subprocess
import time
from pathlib import Path

ROOT=Path(__file__).resolve().parent
PLAN=json.loads((ROOT/'native-link-plan.json').read_text())
STATE=dict(unix=time.time(),phase='preflight',gpu_qualified=False,performance_claimed=False)


def save():
    temporary=ROOT/'native-link-result.tmp'
    temporary.write_text(json.dumps(STATE,indent=2)+'\n')
    temporary.replace(ROOT/'native-link-result.json')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(args,name,cwd=None):
    STATE['step']=name
    save()
    with (ROOT/(name+'.log')).open('xb') as log:
        process=subprocess.run(args,cwd=cwd or PLAN['link_cwd'],stdout=log,stderr=log,timeout=40)
    if process.returncode:
        raise RuntimeError(name+' failed')


try:
    os.umask(0o077)
    os.environ['CUDA_VISIBLE_DEVICES']=''
    save()
    if ROOT.resolve()!=ROOT or ROOT.stat().st_uid!=10250 or os.getuid()!=10250:
        raise RuntimeError('exact native link owner differs')
    proof=json.loads((ROOT.parent/'resource-result.json').read_text())
    if proof['phase']!='passed' or time.time()+180>proof['leaseEndsUnix']:
        raise RuntimeError('finite native link lease insufficient')
    cpu=json.loads((Path(PLAN['cpu_root'])/'build-result.json').read_text())
    if cpu['phase']!='compiled' or not cpu['original_object_byte_equal'] or not cpu['default0_object_byte_equal']:
        raise RuntimeError('original/default0 exact object reproduction required')
    original=Path(cpu['original_archive'])
    candidate=Path(cpu['candidate_object']['path'])
    if sha(original)!=cpu['original_archive_sha256'] or candidate.is_symlink() or sha(candidate)!=cpu['candidate_object']['sha256']:
        raise RuntimeError('exact archive/Engram object differs')
    old=json.loads((Path(PLAN['cpu_root'])/'native34f-link-plan.json').read_text())
    for row in old['inputs']:
        path=Path(row['path'])
        if path.is_symlink() or path.stat().st_size!=row['bytes'] or sha(path)!=row['sha256']:
            raise RuntimeError('qualified existing native link input differs')
    control=list(old['control_link'])
    if control.count(old['original_host_object'])!=1 or control.count(str(original))!=1 or control.count('-o')!=1:
        raise RuntimeError('exact native34f substitution differs')
    control[control.index(old['original_host_object'])]=old['candidate_host_object']
    control[control.index('-o')+1]=str(ROOT/'ds41_generate.control')
    run(control,'exact-native34f-control-link')
    if sha(ROOT/'ds41_generate.control')!=PLAN['control_binary_sha256']:
        raise RuntimeError('control link does not reproduce qualified native34f byte-for-byte')
    STATE['control_binary_byte_equal']=True
    archive=ROOT/'libds41_dense.a'
    shutil.copyfile(original,archive)
    run(['ar','rD',str(archive),str(candidate)],'replace-only-engram-member',ROOT)
    def members(path):
        return subprocess.run(['ar','t',str(path)],capture_output=True,text=True,check=True,timeout=5).stdout.splitlines()
    names=members(original)
    if names!=members(archive) or names.count('ds41_engram.cpp.o')!=1:
        raise RuntimeError('archive member identity/order changed')
    verified=[]
    for name in names:
        left=subprocess.run(['ar','p',str(original),name],capture_output=True,check=True,timeout=5).stdout
        right=subprocess.run(['ar','p',str(archive),name],capture_output=True,check=True,timeout=5).stdout
        if name=='ds41_engram.cpp.o':
            if hashlib.sha256(right).hexdigest()!=cpu['candidate_object']['sha256']:
                raise RuntimeError('replacement member differs from qualified CPU output')
        elif left!=right:
            raise RuntimeError('another dense archive member changed')
        verified.append(dict(member=name,sha256=hashlib.sha256(right).hexdigest(),unchanged=name!='ds41_engram.cpp.o'))
    args=list(control)
    args[args.index(str(original))]=str(archive)
    args[args.index('-o')+1]=str(ROOT/'ds41_generate')
    run(args,'engram-fp8-lut-candidate-link')
    env={key:value for key,value in os.environ.items() if not key.startswith(('STRATA_','ASTER_XQ_'))}
    env['CUDA_VISIBLE_DEVICES']=''
    rejected=[]
    for value in ['', '1','0.10','true','NaN','inf',' 0.1','0.1 ']:
        result=subprocess.run([str(ROOT/'ds41_generate'),'--help'],env=dict(env,ASTER_XQ_DSPARK_CUM=value),capture_output=True,text=True,timeout=5)
        if result.returncode!=2 or result.stderr.strip()!='ASTER_XQ_DSPARK_CUM must be 0, 0.1, 0.25, or 0.5':
            raise RuntimeError('unchanged invalid cumulative mode refusal failed')
        rejected.append(value)
    STATE.update(phase='linked',binary=dict(path=str(ROOT/'ds41_generate'),bytes=(ROOT/'ds41_generate').stat().st_size,sha256=sha(ROOT/'ds41_generate')),
                 archive_members=verified,invalid_startup_modes_rejected=len(rejected),routing_changed=False,
                 gpu_math_executed=False,changed_control='Only Engram FP8 lookup function; original scale arithmetic unchanged',finished_unix=time.time())
except BaseException as error:
    STATE.update(phase='failed',error=type(error).__name__+': '+str(error),finished_unix=time.time())
    raise
finally:
    save()
