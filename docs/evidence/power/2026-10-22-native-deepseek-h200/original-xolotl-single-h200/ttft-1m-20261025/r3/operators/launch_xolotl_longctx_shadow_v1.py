"""Qualify a larger context on the idle owned H200, without moving the serving endpoint."""
import ast,base64,hashlib,json,time
from pathlib import Path
import xolotl_trial_common as trial
from xolotl_serving_common import observe as serving_observe

BASE=trial.BASE
ip=BASE/'xolotl-longctx-shadow-v1-intent.json'
if ip.exists():raise RuntimeError('long-context intent exists; inspect it instead of relaunching')
old=json.loads((BASE/'xolotl-stable-service-v2-intent.json').read_text())
o=trial.observe();s=serving_observe();lease=trial.lease()
if o['job_uid']!='81f6b9d4-409d-4ec2-98cf-cd402e6a380b' or o['phase']!='Running' or o.get('resource-result.json',{}).get('phase')!='passed' or s['job_uid']!=old['allocation_uid'] or s['phase']!='Running':raise RuntimeError('two exact owned allocations must be running')
gpu=o['resource-result.json']['gpus'][0]['uuid']
if gpu==old['gpu_uuid']:raise RuntimeError('shadow and live serving must use different physical GPUs')

# Pin the shadow transport before reserving the former optimization allocation.
common=(BASE/'xolotl_serving_common.py').read_text()
common=common.replace('xolotl-single-serving-lease-pending.json','xolotl-single500-cont5-lease-pending.json').replace('xolotl-serving-resource-latest.json','xolotl-longctx-resource-latest.json')
ast.parse(common);(BASE/'xolotl_longctx_common.py').write_text(common)
import xolotl_longctx_common as shadow
shadow.observe()
reservation=dict(schema='aster-serving-reservation-v1',unix=time.time(),allocation_uid=o['job_uid'],gpu_uuid=gpu,purpose='131072-context shadow qualification; optimization is prohibited while reserved',shadow_intent=ip.name)
(BASE/'xolotl-active-serving-reservation.json').write_text(json.dumps(reservation,indent=2)+'\n')
transport=BASE/'xolotl_trial_common.py';text=transport.read_text()
anchor="    replica=observation['replicas'][0]\n"
guard="""    reservation_path=BASE/'xolotl-active-serving-reservation.json'
    if reservation_path.exists():
        reservation=json.loads(reservation_path.read_text())
        if reservation['allocation_uid']==observation['job_uid']:
            raise RuntimeError('optimization refused: allocation is reserved for authenticated serving qualification')
"""
if text.count(anchor)!=1 or 'optimization refused: allocation is reserved' in text:raise RuntimeError('optimization reservation guard anchor differs')
text=text.replace(anchor,guard+anchor);ast.parse(text);transport.write_text(text)

root=lease['remote']+'/serve-long-context-v1'
plan=dict(old['plan']);plan.update(schema='aster-xolotl-long-context-serving-plan-v1',lease_ends_unix=o['resource-result.json']['leaseEndsUnix'],optimization_gpu_uuid=old['gpu_uuid'],context_tokens=131072)
config_code='''import json
from pathlib import Path
p=Path(@ROOT@)/'server-config.json'
if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>16384:raise RuntimeError('old public configuration differs')
print(json.dumps(json.loads(p.read_text())))
'''.replace('@ROOT@',repr(old['root']))
config=trial.jump(config_code);config['args'][config['args'].index('--ctx')+1]='131072';config.update(cwd=root,tokenizer=root+'/tokenizer',log=root+'/engine.log')
worker=(BASE/'xolotl_stable_service_worker_v2.frozen.py').read_text().replace('service-v2-result','service-longctx-v1-result').replace('api-v2-attempt','api-longctx-v1-attempt')
worker=worker.replace("STATE.update(phase='starting_api',", "STATE.update(context_tokens=PLAN['context_tokens'],phase='starting_api',")
ast.parse(worker);(BASE/'xolotl_longctx_shadow_worker_v1.frozen.py').write_text(worker)
payload={'service-worker.py':worker.encode(),'service-plan.json':(json.dumps(plan,indent=2)+'\n').encode(),'server-config.json':(json.dumps(config,indent=2)+'\n').encode(),'prompt-id-controls.json':(BASE/'xolotl-trial-prompt-ids.json').read_bytes()}
r=dict(unix=time.time(),phase='prepared',root=root,source_public_root=old['root'],allocation_job=o['job'],allocation_uid=o['job_uid'],replica=o['replicas'][0]['name'],replica_uid=o['replicas'][0]['uid'],gpu_uuid=gpu,pod_ip=o['replicas'][0]['status']['podIP'],host_ip=o['replicas'][0]['status']['hostIP'],plan=plan,credential_file=old['credential_file'],purpose='Validate 35700-token Workbuddy requests on a shadow GPU before restarting the original stable pod at its same address',files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in payload.items()])
ip.write_text(json.dumps(r,indent=2)+'\n')
code='''import base64,hashlib,json,os,shutil,subprocess,time
from pathlib import Path,PurePosixPath
r=@R@;root=Path(r['root']);src=Path(r['source_public_root'])
if root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists() or src.resolve()!=src or src.stat().st_uid!=10250:raise RuntimeError('shadow stage owner/exclusivity differs')
root.mkdir(mode=0o700)
manifest=json.loads((src/'public-manifest.json').read_text());rows=list(manifest['files'])
rows += [dict(file='source/serve/chat_template.jinja',bytes=9993,sha256='12827f24b742ea4e80cdc12dbcf9622227056b9f797252a3149263d4f9aaadce'),dict(file='source/serve/chat_golden.json',bytes=12284,sha256='b9006c519218fcb7e5e38ba603905029f79c5c53431b3c9049466a4b0f3bd96f')]
total=0
for row in rows:
 name=PurePosixPath(row['file']);p=src/row['file'];total+=row['bytes']
 if name.is_absolute() or '..' in name.parts or p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or row['bytes']>4*1024**2 or total>24*1024**2:raise RuntimeError('shadow public source owner/bound differs')
 raw=p.read_bytes()
 if hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('shadow public source SHA differs')
 dest=root/row['file'];dest.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
 with dest.open('xb') as f:f.write(raw)
(root/'public-manifest.json').write_text(json.dumps(manifest,indent=2)+'\\n')
data=@DATA@
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('shadow frozen input differs')
 with (root/row['file']).open('xb') as f:f.write(raw)
# Carry the existing authorized service credential only between private runtime directories.
p=src/'credentials/api-key.txt'
if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>256:raise RuntimeError('private source credential owner/bound differs')
secret=p.read_bytes()
if hashlib.sha256(secret).hexdigest()!=r['plan']['credential_file_sha256']:raise RuntimeError('existing authorized credential SHA differs')
(root/'credentials').mkdir(mode=0o700)
with (root/'credentials/api-key.txt').open('xb') as f:f.write(secret)
(root/'credentials/api-key.txt').chmod(0o600)
deadline=int(r['plan']['lease_ends_unix']-time.time()-10)
if deadline<1800:raise RuntimeError('shadow context qualification deadline insufficient')
with (root/'service-worker.log').open('xb') as log:
 child=subprocess.Popen(['timeout','--signal=TERM','--kill-after=30',str(deadline),'python3','-u',str(root/'service-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
p=dict(pid=child.pid,start_ticks=int(Path('/proc/'+str(child.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),deadline_seconds=deadline)
(root/'service-process.json').write_text(json.dumps(p,indent=2)+'\\n');print(json.dumps(p))
'''.replace('@R@',repr(r)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in payload.items()}))
compile(code,'shadow-long-context-bootstrap','exec');r['phase']='launch_requested';ip.write_text(json.dumps(r,indent=2)+'\n');r['process']=shadow.owned(code);r['phase']='launched';ip.write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps(dict(phase=r['phase'],context_tokens=131072,gpu_uuid=gpu,pod_ip=r['pod_ip'],same_key=True,old_endpoint_preserved=True)))
