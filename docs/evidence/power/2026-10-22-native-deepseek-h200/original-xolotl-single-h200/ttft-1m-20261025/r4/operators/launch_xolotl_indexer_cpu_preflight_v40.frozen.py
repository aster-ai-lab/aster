"""Compile on the jump CPU while the independent H200 completes its1M request."""
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, lease, observe, jump

ip=BASE/'xolotl-indexer-cpu-preflight-v40-intent.json'
if ip.exists():raise RuntimeError('V40 CPU preflight already has an exact handle')
o=observe();proof=o['resource-result.json']
reservation=json.loads((BASE/'xolotl-active-serving-reservation.json').read_text())
if o['job_uid']==reservation['allocation_uid']:raise RuntimeError('CPU preflight is bound to the optimization role only')
source=BASE/'indexer-v40';manifest=json.loads((source/'source-manifest.json').read_text())
files={}
for row in manifest['source_files']:
    raw=(source/row['file']).read_bytes()
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen indexer source differs')
    files[row['file']]=raw
files['source-manifest.json']=(source/'source-manifest.json').read_bytes()
files['cpu_preflight_worker.py']=(source/'cpu_preflight_worker.py').read_bytes()
meta=json.loads((BASE/'xolotl-indexer-v40-parent-provenance.json').read_text())['parents'][0]
legacy=json.loads((BASE/'xolotl-dense-chain-parent.json').read_text())
command=legacy['compile_command'].replace(legacy['parent'],meta['root'])
plan=dict(parent=meta['root'],archive_sha256=meta['archive_sha256'],source_sha256=meta['source_sha256'],compile_command=command,sources=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in files.items()])
files['cpu-plan.json']=(json.dumps(plan,indent=2)+'\n').encode()
r=dict(unix=time.time(),phase='prepared',root=lease()['remote']+'/indexer-cpu-preflight-v40',allocation_uid=o['job_uid'],gpu_uuid=proof['gpus'][0]['uuid'],execution_location='jump host CPU only',gpu_executed=False,files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in files.items()])
with ip.open('x') as f:f.write(json.dumps(r,indent=2)+'\n')
code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive CPU preflight owner/root differs')
root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('indexer CPU stage digest differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
with (root/'cpu-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','180','python3','-u',str(root/'cpu_preflight_worker.py')],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=180)
(root/'cpu-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@',repr(r)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in files.items()}))
r['phase']='launch_requested';ip.write_text(json.dumps(r,indent=2)+'\n')
try:
    r['process']=jump(code);r['phase']='launched'
except BaseException:
    r['phase']='launch_outcome_requires_inspection';ip.write_text(json.dumps(r,indent=2)+'\n');raise
ip.write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps({'phase':r['phase'],'process':r['process'],'execution_location':r['execution_location'],'full1m_gpu_request_unchanged':True}))
