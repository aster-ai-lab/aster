"""GPU-free controlled relink and host compilation; original files remain immutable."""
import hashlib,json,os,shlex,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parent;P=json.loads((ROOT/'plan.json').read_text())
STATE=dict(unix=time.time(),phase='preflight',gpu_tests_claimed=False,promoted=False)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save():
    p=ROOT/'result.tmp';p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(ROOT/'result.json')
def run(args,name,seconds,cwd):
    STATE['step']=name;save()
    with (ROOT/(name+'.log')).open('xb') as log:q=subprocess.run(args,cwd=cwd,stdout=log,stderr=log,timeout=seconds)
    if q.returncode:raise RuntimeError(name+' exit '+str(q.returncode))
try:
    os.umask(0o077);os.environ['CUDA_VISIBLE_DEVICES']='';save()
    if os.getuid()!=10250 or ROOT.stat().st_uid!=10250:raise RuntimeError('build owner differs')
    proof=json.loads((ROOT.parent/'resource-result.json').read_text())
    if proof['phase']!='passed' or time.time()+240>proof['leaseEndsUnix']:raise RuntimeError('bounded build lease insufficient')
    parent=Path(P['parent']);source=Path(P['source_path'])
    if sha(source)!=P['source_sha256'] or sha(parent/'build/ds41_generate')!=P['stable_binary_sha256']:raise RuntimeError('stable parent identity differs')
    for row in P['sources']:
        p=ROOT/row['file']
        if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or sha(p)!=row['sha256']:raise RuntimeError('frozen build input differs')
    command=P['link_command']
    if not command.startswith(': && ') or not command.endswith(' && :'):raise RuntimeError('exact link wrapper differs')
    link=shlex.split(command[5:-5]);object_name='CMakeFiles/ds41_generate.dir/tools/v41/engine/ds41_generate.cpp.o'
    if link.count(object_name)!=1 or link.count('-o')!=1:raise RuntimeError('exact link inputs differ')
    # Reproduce the qualified executable using the exact retained object/archive set.
    control=list(link);control[control.index('-o')+1]=str(ROOT/'ds41_generate.control')
    run(control,'control-relink',30,parent/'build')
    if sha(ROOT/'ds41_generate.control')!=P['stable_binary_sha256']:raise RuntimeError('control relink does not reproduce stable executable')
    STATE['control_binary_sha256']=sha(ROOT/'ds41_generate.control');save()
    args=shlex.split(P['compile_command']);clean=[];i=0
    while i<len(args):
        if args[i] in ['-MT','-MF']:i+=2;continue
        if args[i]=='-MD':i+=1;continue
        clean.append(args[i]);i+=1
    clean[clean.index('-o')+1]=str(ROOT/'ds41_generate.cpp.o');clean[clean.index('-c')+1]=str(ROOT/'ds41_generate.cpp')
    run(clean,'candidate-compile',90,ROOT)
    run(['/usr/bin/c++','-O2','-std=c++20',str(ROOT/'policy_test.cpp'),'-o',str(ROOT/'policy_test')],'policy-compile',15,ROOT)
    run([str(ROOT/'policy_test')],'policy-test',10,ROOT)
    candidate=list(link);candidate[candidate.index(object_name)]=str(ROOT/'ds41_generate.cpp.o');candidate[candidate.index('-o')+1]=str(ROOT/'ds41_generate')
    run(candidate,'candidate-link',30,parent/'build')
    STATE.update(phase='built',policy_test_passed=True,binary=dict(path=str(ROOT/'ds41_generate'),bytes=(ROOT/'ds41_generate').stat().st_size,sha256=sha(ROOT/'ds41_generate')))
except BaseException as e:
    STATE.update(phase='failed',error=type(e).__name__+': '+str(e));raise
finally:
    STATE['finished_unix']=time.time();save()
