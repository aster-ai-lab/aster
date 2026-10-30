"""Execute hash-checked owned bootstrap files without long websocket exec query arguments."""
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease, observe, owned

LABELS={'topk-native-v212-control_a','topk-native-v212-topk','topk-native-v212-control_b'}

def sealed_owned(code,label):
    if label not in LABELS:raise RuntimeError('closed sealed bootstrap label differs')
    raw=code.encode('utf-8',errors='strict')
    if not 0<len(raw)<=1024**2:raise RuntimeError('bounded sealed bootstrap size differs')
    key=(BASE/'access-private-xolotl-9c996c5a434b/api-key.txt').read_bytes().strip()
    if not key or key in raw:raise RuntimeError('actual credential negative control failed')
    o=observe()
    if o['phase']!='Running' or o.get('resource-result.json',{}).get('phase')!='passed':
        raise RuntimeError('fresh exact owned optimization resource required')
    intent=BASE/('xolotl-sealed-'+label+'-intent.json')
    if intent.exists():raise RuntimeError('sealed bootstrap has an exact handle; observe it')
    path=lease()['remote']+'/sealed-'+label+'.py'
    record=dict(unix=time.time(),phase='prepared',label=label,path=path,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),
                allocation_uid=o['job_uid'],credentials_copied=False,inline_exec_payload_replaced=True)
    with intent.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
    stage='''import base64,hashlib,json,os
from pathlib import Path
r=@R@;p=Path(r['path']);raw=base64.b64decode(@DATA@)
if p.parent.resolve()!=p.parent or p.parent.stat().st_uid!=10250 or p.exists():raise RuntimeError('exclusive sealed bootstrap owner differs')
if len(raw)!=r['bytes'] or hashlib.sha256(raw).hexdigest()!=r['sha256']:raise RuntimeError('exact sealed bootstrap bytes differ')
os.umask(0o077)
with p.open('xb') as output:output.write(raw)
print(json.dumps(dict(staged=True,bytes=len(raw),sha256=r['sha256'])))
'''.replace('@R@',repr(record)).replace('@DATA@',repr(base64.b64encode(raw).decode()))
    try:
        record['stage']=jump(stage,80);record['phase']='staged';intent.write_text(json.dumps(record,indent=2)+'\n')
        execute='''import hashlib
from pathlib import Path
p=Path(@PATH@)
if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=@SIZE@ or hashlib.sha256(p.read_bytes()).hexdigest()!=@SHA@:raise RuntimeError('exact sealed bootstrap activation refused')
exec(compile(p.read_text(),str(p),'exec'))
'''.replace('@PATH@',repr(path)).replace('@SIZE@',repr(len(raw))).replace('@SHA@',repr(record['sha256']))
        result=owned(execute);record['phase']='executed';record['result']=result
    except BaseException:
        record['phase']='activation_outcome_requires_inspection';intent.write_text(json.dumps(record,indent=2)+'\n');raise
    intent.write_text(json.dumps(record,indent=2)+'\n')
    return result
