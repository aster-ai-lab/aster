"""Stage a separate formal-service replacement and run CPU checks before any running process changes."""
import ast,base64,hashlib,json,time
from pathlib import Path
from xolotl_trial_common import BASE,jump

old=json.loads((BASE/'xolotl-stable-service-v279-intent.json').read_text())
root=str(Path(old['root']).parent).replace('\\','/')+'/serve-tool-repair-v284'
candidate=BASE/'tool-adapter-v267r3'
worker=(BASE/'xolotl_qualified_api_worker_v53.frozen.py').read_bytes()
anchor=b" STATE.update(context_tokens=PLAN['context_tokens'],phase='starting_api',"
if worker.count(anchor)!=1:raise RuntimeError('exact worker check insertion differs')
worker=worker.replace(anchor,b" run([sys.executable,str(ROOT/'test_tool_roundtrip.py'),str(ROOT)],'tool-roundtrip-tests-v284',30,env)\n"+anchor)
tree=ast.parse((BASE/'test_tool_roundtrip_v268.py').read_text())
selected=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in ['history','server'] or isinstance(n,ast.ClassDef) and n.name=='ProtocolTests' or isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='TOOLS' for t in n.targets)]
if len(selected)!=4:raise RuntimeError('exact hermetic test extraction differs')
tests='''"""Real full-module tool roundtrip checks, no Jinja substitutes and no model loading."""
import copy,json,sys,types,unittest
from pathlib import Path
ROOT=Path(sys.argv.pop(1));sys.path.insert(0,str(ROOT/'source/tools/v41/serve'))
import serve_ds41 as D
import aster_tool_roundtrip as G
F,E=D.F,D.E
'''+ast.unparse(ast.Module(body=selected,type_ignores=[]))+"\nif __name__=='__main__':unittest.main(verbosity=2)\n"
files={'service-worker.py':worker,'test_tool_roundtrip.py':tests.encode(),
 'source/serve/frontend.py':(candidate/'frontend.py').read_bytes(),
 'source/tools/v41/serve/serve_ds41.py':(candidate/'serve_ds41.py').read_bytes(),
 'source/tools/v41/serve/aster_tool_roundtrip.py':(candidate/'aster_tool_roundtrip.py').read_bytes()}
for name in ['test_exact_token_cache.py','test_strict_native_json.py','test_combined_adapter.py','test_hybrid_native_json.py','actual-tool-output.json']:
 files[name]=(BASE/'api-adapter-v52'/name).read_bytes()
files['prompt-id-controls.json']=(BASE/'xolotl-trial-prompt-ids.json').read_bytes()
plan=json.loads(json.dumps(old['plan']));plan.update(schema='aster-xolotl-formal-tool-repair-v284',api_overlay='tool-adapter-v267r3',same_qualified_native=True)
for item in plan['inputs']:
 if item['path'].startswith(old['root']+'/'):
  item['path']=root+item['path'][len(old['root']):]
for name in ['source/serve/frontend.py','source/tools/v41/serve/serve_ds41.py','source/tools/v41/serve/aster_tool_roundtrip.py','test_tool_roundtrip.py']:
 b=files[name];path=root+'/'+name
 plan['inputs']=[i for i in plan['inputs'] if i['path']!=path]
 plan['inputs'].append(dict(path=path,bytes=len(b),sha256=hashlib.sha256(b).hexdigest(),verify_small_sha256=True))
files['service-plan.json']=(json.dumps(plan,indent=2)+'\n').encode()
record=dict(unix=time.time(),phase='prepare_requested',root=root,old_root=old['root'],
 allocation_uid=old['allocation_uid'],replica_uid=old['replica_uid'],gpu_uuid=old['gpu_uuid'],pod_ip=old['pod_ip'],
 native_sha256=plan['binary_sha256'],lease_ends_unix=plan['lease_ends_unix'],same_vip=True,same_existing_key=True,
 files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in files.items()])
ip=BASE/'xolotl-formal-tool-repair-v284-intent.json'
with ip.open('x') as f:f.write(json.dumps(record,indent=2)+'\n')
code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path,PurePosixPath
r=@R@;root=Path(r['root']);src=Path(r['old_root']);data=@DATA@
if root.parent.resolve()!=root.parent or root.exists() or src.resolve()!=src or src.stat().st_uid!=10250:raise RuntimeError('exact isolated patch stage differs')
root.mkdir(mode=0o700)
manifest=json.loads((src/'public-manifest.json').read_bytes());rows=manifest['files'];total=0
overrides={row['file']:row for row in r['files'] if row['file'].startswith('source/')}
seen=set();updated=[]
for row in rows:
 name=PurePosixPath(row['file']);p=src/row['file'];total+=row['bytes']
 if name.is_absolute() or '..' in name.parts or p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or row['bytes']>4*1024**2 or total>24*1024**2:raise RuntimeError('bounded pinned public source differs')
 raw=p.read_bytes()
 if hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('pinned old public source changed')
 if row['file'] in overrides:
  new=overrides[row['file']];raw=base64.b64decode(data[row['file']]);row=dict(file=row['file'],bytes=new['bytes'],sha256=new['sha256']);seen.add(row['file'])
 dest=root/row['file'];dest.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
 with dest.open('xb') as f:f.write(raw)
 updated.append(row)
for row in r['files']:
 name=PurePosixPath(row['file'])
 if name.is_absolute() or '..' in name.parts:raise RuntimeError('payload path is not relative')
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact candidate bytes changed')
 if row['file'] in seen:continue
 dest=root/row['file'];dest.parent.mkdir(mode=0o700,parents=True,exist_ok=True)
 with dest.open('xb') as f:f.write(raw)
 if row['file'].startswith('source/'):updated.append(dict(file=row['file'],bytes=row['bytes'],sha256=row['sha256']))
for row in [dict(file='source/serve/chat_template.jinja',bytes=9993,sha256='12827f24b742ea4e80cdc12dbcf9622227056b9f797252a3149263d4f9aaadce'),dict(file='source/serve/chat_golden.json',bytes=12284,sha256='b9006c519218fcb7e5e38ba603905029f79c5c53431b3c9049466a4b0f3bd96f')]:
 dest=root/row['file']
 if dest.exists():continue
 raw=(src/row['file']).read_bytes()
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact supplemental template differs')
 with dest.open('xb') as f:f.write(raw)
manifest['files']=updated
with (root/'public-manifest.json').open('x') as f:f.write(json.dumps(manifest,indent=2)+'\\n')
config=json.loads((src/'server-config.json').read_bytes());config.update(cwd=str(root),tokenizer=str(root/'tokenizer'),log=str(root/'engine.log'))
with (root/'server-config.json').open('x') as f:f.write(json.dumps(config,indent=2)+'\\n')
p=src/'credentials/api-key.txt'
if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>256:raise RuntimeError('exact authorized credential differs')
secret=p.read_bytes();plan=json.loads((root/'service-plan.json').read_bytes())
if hashlib.sha256(secret).hexdigest()!=plan['credential_file_sha256']:raise RuntimeError('authorized key SHA differs')
(root/'credentials').mkdir(mode=0o700)
with (root/'credentials/api-key.txt').open('xb') as f:f.write(secret)
(root/'credentials/api-key.txt').chmod(0o600)
env=dict(os.environ,PYTHONPATH=str(src/'python-deps-v2')+os.pathsep+str(root/'source')+os.pathsep+str(root/'source/tools/v41/serve'),PYTHONDONTWRITEBYTECODE='1')
tests=[(['python3',str(root/'test_tool_roundtrip.py'),str(root)],'tool-roundtrip-tests'),(['python3','-m','unittest','serve.test_security'],'security-tests'),(['python3',str(root/'source/tools/v41/serve/test_serve_ds41.py'),str(root/'tokenizer')],'ds41-api-tests')]
out=[]
for command,name in tests:
 p=subprocess.run(command,cwd=root/'source',env=env,capture_output=True,text=True,timeout=40)
 if len(p.stdout)+len(p.stderr)>65536:raise RuntimeError('bounded CPU test output exceeded')
 with (root/(name+'.cpu-preflight.log')).open('x') as f:f.write(p.stdout+p.stderr)
 out.append(dict(name=name,returncode=p.returncode,tail=(p.stdout+p.stderr)[-2000:]))
 if p.returncode:break
receipt=dict(unix=time.time(),phase='cpu_qualified' if all(t['returncode']==0 for t in out) and len(out)==3 else 'cpu_failed',tests=out,formal_service_changed=False,model_inference_sent=False)
with (root/'cpu-preflight.json').open('x') as f:f.write(json.dumps(receipt,indent=2)+'\\n')
print(json.dumps(receipt))
'''.replace('@R@',repr(record)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in files.items()}))
compile(code,'formal-tool-repair-v284-prepare','exec')
record['preflight']=jump(code,55);record['phase']=record['preflight']['phase']
ip.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record['preflight'],indent=2))
