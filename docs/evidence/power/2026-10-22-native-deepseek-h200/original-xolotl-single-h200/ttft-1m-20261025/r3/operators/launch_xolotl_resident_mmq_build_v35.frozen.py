"""Build the opt-in resident MMQ host path on the independent verified H200 job."""
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, lease, observe, jump, owned

ip = BASE/'xolotl-resident-mmq-build-v35-intent.json'
if ip.exists():
    raise RuntimeError('V35 build already has an exact handle; inspect it')
o = observe()
stable = json.loads((BASE/'xolotl-stable-service-v31-intent.json').read_text())
if o['job_uid'] == stable['allocation_uid'] or o['resource-result.json']['gpus'][0]['uuid'] == stable['gpu_uuid']:
    raise RuntimeError('independent optimization resource required')
recipe = json.loads((BASE/'xolotl-mixed-keep-v19-build-recipe.json').read_text())
link = json.loads((BASE/'xolotl-ttft-v11-build-recipe.json').read_text())
prior = json.loads((BASE/'xolotl-ttft-build-v15-intent.json').read_text())
source = BASE/'resident-mmq-v35'
manifest = json.loads((source/'source-manifest.json').read_text())
files = {}
for row in manifest['files']:
    raw = (source/row['file']).read_bytes()
    if len(raw) != row['bytes'] or hashlib.sha256(raw).hexdigest() != row['sha256']:
        raise RuntimeError('frozen resident MMQ input differs')
    files[row['file']] = raw
files['source-manifest.json'] = (source/'source-manifest.json').read_bytes()
plan = dict(recipe, link_command=link['link_command'], v15_build_root=prior['root'], v15_binary_sha256=stable['plan']['binary_sha256'], sources=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in files.items()])
files['plan.json'] = (json.dumps(plan,indent=2)+'\n').encode()
r = dict(unix=time.time(),phase='prepared',root=lease()['remote']+'/build-resident-mmq-v35',allocation_uid=o['job_uid'],gpu_uuid=o['resource-result.json']['gpus'][0]['uuid'],gpu_free=True,formal_service_unchanged=True,files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in files.items()])
with ip.open('x') as f:
    f.write(json.dumps(r,indent=2)+'\n')
code = '''import base64,hashlib,json,os
from pathlib import Path
r=@R@;data=@DATA@;root=Path(r['root'])
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive V35 build root differs')
root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('V35 source digest differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
print(json.dumps({'staged':True,'files':len(r['files'])}))
'''.replace('@R@',repr(r)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in files.items()}))
r['stage'] = jump(code)
r['phase'] = 'staged'
ip.write_text(json.dumps(r,indent=2)+'\n')
code = '''import json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root'])
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('V35 build root owner differs')
with (root/'worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','300','python3','-u',str(root/'build_worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=300)
(root/'process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@',repr(r))
r['phase'] = 'launch_requested'
ip.write_text(json.dumps(r,indent=2)+'\n')
try:
    r['process'] = owned(code)
    r['phase'] = 'launched'
except BaseException:
    r['phase'] = 'launch_outcome_requires_inspection'
    ip.write_text(json.dumps(r,indent=2)+'\n')
    raise
ip.write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps({'phase':r['phase'],'root':r['root'],'process':r['process'],'formal_service_unchanged':True}))
