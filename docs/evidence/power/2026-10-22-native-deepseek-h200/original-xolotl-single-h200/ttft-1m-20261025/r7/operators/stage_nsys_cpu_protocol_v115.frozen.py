"""Repeat the already proven profiler control protocol on an owned GPU-hidden CPU child."""
import ast
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, lease, observe, owned

intent=BASE/'xolotl-nsys-cpu-protocol-v115-intent.json'
if intent.exists():raise RuntimeError('exact CPU protocol handle exists; observe it')
parent=(BASE/'probe_native_nsys_interactive.py').read_text()
values=[n.value for n in ast.walk(ast.parse(parent)) if isinstance(n,ast.Constant) and isinstance(n.value,str)
        and 'owned profiler protocol root changed' in n.value and 'launch_waited_for_target' in n.value]
if len(values)!=1:raise RuntimeError('exact previously proven profiler protocol source differs')
source=values[0].replace('service=Path(ROOT)','service=Path('+repr(lease()['remote'])+')')
for a,b in [("root=service/'nsys-interactive-cpu-probe'","root=service/'nsys-interactive-cpu-v115'"),
            ("session='aster-v24-cpu-protocol-'","session='aster-h200-cpu-v115-'"),
            ("'--trace=cuda'","'--trace=osrt'"),
            ("os.umask(0o077);root=", "os.environ['CUDA_VISIBLE_DEVICES']='';os.umask(0o077);root=")]:
    if source.count(a)!=1:raise RuntimeError('pinned profiler control anchor differs')
    source=source.replace(a,b)
source=source.replace("print(json.dumps(dict(result=result,logs={p.name:p.read_text() for p in root.glob('*.log') if p.stat().st_size<65536})))",
                      "print(json.dumps(dict(passed=result.get('passed',False),session=session,model_inference_performed=False)))")
compile(source,'nsys-cpu-protocol-v115','exec')
root=lease()['remote']+'/nsys-cpu-protocol-v115-launch'
r=dict(unix=time.time(),phase='prepared',root=root,gpu_executed=False,credentials_copied=False,
       worker_sha256=hashlib.sha256(source.encode()).hexdigest(),worker_bytes=len(source.encode()))
o=observe()
if o['job_uid']=='3a66564e-0a12-4c17-9b41-741fd78e0017':raise RuntimeError('formal service cannot be a profiler target')
code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);raw=base64.b64decode(@SOURCE@)
if root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive CPU protocol root differs')
if len(raw)!=r['worker_bytes'] or hashlib.sha256(raw).hexdigest()!=r['worker_sha256']:raise RuntimeError('exact CPU protocol source differs')
root.mkdir(mode=0o700);(root/'worker.py').write_bytes(raw)
with (root/'worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','90','python3','-u',str(root/'worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''))
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=90,namespace='optimization pod')
(root/'process.json').write_text(json.dumps(process,indent=2)+chr(10));print(json.dumps(process))
'''.replace('@R@',repr(r)).replace('@SOURCE@',repr(base64.b64encode(source.encode()).decode()))
compile(code,'nsys-cpu-protocol-stage-v115','exec')
with intent.open('x') as output:output.write(json.dumps(r,indent=2)+'\n')
r['phase']='launch_requested';intent.write_text(json.dumps(r,indent=2)+'\n')
try:r['process']=owned(code);r['phase']='launched'
except BaseException:
    r['phase']='launch_outcome_requires_inspection';intent.write_text(json.dumps(r,indent=2)+'\n');raise
intent.write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps(dict(phase=r['phase'],process=r['process'],gpu_executed=False)))
