"""Stage public-only files over stdin, then launch bounded GPU-free compilation."""
import base64,hashlib,json,time
from xolotl_trial_common import BASE,lease,observe,jump,owned
ip=BASE/'xolotl-ttft-build-v15-intent.json'
if ip.exists():raise RuntimeError('V11 build intent exists; observe exact process')
o=observe();recipe=json.loads((BASE/'xolotl-ttft-v11-build-recipe.json').read_text());stable=json.loads((BASE/'xolotl-stable-service-v8-intent.json').read_text())
if o['job_uid']==stable['allocation_uid']:raise RuntimeError('formal serving allocation refused')
local=BASE/'ttft-v15';files={n:(local/n).read_bytes() for n in ['ds41_generate.cpp','aster_prefill_policy.hpp','policy_test.cpp','build_worker.py','prefill-v11.patch']}
plan=dict(recipe,stable_binary_sha256=stable['plan']['binary_sha256'],sources=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in files.items()])
files['plan.json']=(json.dumps(plan,indent=2)+'\n').encode();r=dict(unix=time.time(),phase='prepared',root=lease()['remote']+'/build-ttft-v15',allocation_uid=o['job_uid'],files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in files.items()]);ip.write_text(json.dumps(r,indent=2)+'\n')
code='''import base64,hashlib,json,os
from pathlib import Path
r=@R@;data=@DATA@;root=Path(r['root'])
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive owned build root differs')
root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen source changed')
 with (root/row['file']).open('xb') as f:f.write(raw)
print(json.dumps(dict(staged=True,files=len(r['files']))))
'''.replace('@R@',repr(r)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in files.items()}))
r['stage']=jump(code);r['phase']='staged';ip.write_text(json.dumps(r,indent=2)+'\n')
code='''import json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root'])
if root.stat().st_uid!=10250 or root.is_symlink():raise RuntimeError('build root owner differs')
with (root/'worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','210','python3','-u',str(root/'build_worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
v=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=210)
(root/'process.json').write_text(json.dumps(v,indent=2)+'\\n');print(json.dumps(v))
'''.replace('@R@',repr(r))
r['phase']='launch_requested';ip.write_text(json.dumps(r,indent=2)+'\n');r['process']=owned(code);r['phase']='launched';ip.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(dict(phase='launched',gpu_free=True,root=r['root'])))
