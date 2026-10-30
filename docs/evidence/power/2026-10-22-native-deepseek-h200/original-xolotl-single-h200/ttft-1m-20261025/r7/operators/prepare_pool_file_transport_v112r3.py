"""Preserve the bootstrap byte-for-byte; stage it over SSH stdin to avoid an oversized exec URI."""
import hashlib
import json
from xolotl_trial_common import BASE

parent=BASE/'launch_pool_service_v108.frozen.py';source=parent.read_text()
before="intent=BASE/('xolotl-pool-service-v108-'+profile+'-intent.json')"
if source.count(before)!=1:raise RuntimeError('pinned profile intent anchor differs')
source=source.replace(before,"intent=BASE/('xolotl-pool-service-v108-'+profile+'-retry-v112r3-intent.json')")
before="compile(code,'exact-independent-isolation-bootstrap-v89','exec')"
if source.count(before)!=1:raise RuntimeError('pinned bootstrap boundary differs')
addition='''
if profile!='pool_off_repeat':raise RuntimeError('file transport is scoped to the exact unfinished repeat')
from xolotl_trial_common import jump
transport=BASE/'xolotl-pool-file-transport-v112r3-intent.json'
if transport.exists():raise RuntimeError('file transport has an exact handle; inspect it')
payload=code.encode();bootstrap=lease()['remote']+'/pool-repeat-bootstrap-v112r3.py'
trans=dict(unix=time.time(),phase='prepared',bootstrap=bootstrap,sha256=hashlib.sha256(payload).hexdigest(),bytes=len(payload),
    same_bootstrap_bytes=True,allocation_uid=record['allocation_uid'],model_launched=False)
with transport.open('x') as output:output.write(json.dumps(trans,indent=2)+'\\n')
stage="""import base64,hashlib,json,os
from pathlib import Path
r=@TRANS@;p=Path(r['bootstrap']);root=Path(@MODELROOT@);data=base64.b64decode(@PAYLOAD@)
if p.parent.resolve()!=p.parent or p.parent.stat().st_uid!=10250 or p.exists() or root.exists():raise RuntimeError('exact unfinished bootstrap/model must not exist')
if len(data)!=r['bytes'] or hashlib.sha256(data).hexdigest()!=r['sha256']:raise RuntimeError('exact original bootstrap bytes differ')
os.umask(0o077)
with p.open('xb') as output:output.write(data)
print(json.dumps(dict(staged=True,root_exists=root.exists(),bytes=len(data),sha256=r['sha256'])))
""".replace('@TRANS@',repr(trans)).replace('@MODELROOT@',repr(record['root'])).replace('@PAYLOAD@',repr(base64.b64encode(payload).decode()))
trans['staging']=jump(stage);trans['phase']='staged';transport.write_text(json.dumps(trans,indent=2)+'\\n')
code="""import hashlib
from pathlib import Path
p=Path(@BOOT@)
if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=@SIZE@ or hashlib.sha256(p.read_bytes()).hexdigest()!=@SHA@:raise RuntimeError('exact sealed original bootstrap differs')
exec(compile(p.read_text(),str(p),'exec'))
""".replace('@BOOT@',repr(bootstrap)).replace('@SIZE@',repr(len(payload))).replace('@SHA@',repr(trans['sha256']))
compile(code,'sealed-bootstrap-file-transport-v112r3','exec')
'''
source=source.replace(before,before+'\n'+addition)
compile(source,'launch_pool_service_file_v112r3.frozen.py','exec')
path=BASE/'launch_pool_service_file_v112r3.frozen.py'
with path.open('x',encoding='utf-8',newline='\n') as output:output.write(source)
print(json.dumps(dict(file=path.name,parent_sha256=hashlib.sha256(parent.read_bytes()).hexdigest(),sha256=hashlib.sha256(path.read_bytes()).hexdigest(),gpu_launched=False)))
