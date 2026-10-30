"""Identical full-window cache-hit load after the exact packed cold gate finishes."""
import ast
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, observe, jump
ip=BASE/'xolotl-resident-fullwindow-load-v41-intent.json'
if ip.exists():raise RuntimeError('V41 load has an exact handle; observe it')
r=json.loads((BASE/'xolotl-resident-compare-v37-v35_packed-intent.json').read_text())
full=json.loads((BASE/'xolotl-resident-v38-fullwindow-intent.json').read_text())
o=observe();reservation=json.loads((BASE/'xolotl-active-serving-reservation.json').read_text())
if o['job_uid']!=r['allocation_uid'] or r['allocation_uid']==reservation['allocation_uid'] or o['phase']!='Running':raise RuntimeError('exact independent packed optimization service required')
plan=dict(base_url='http://'+r['pod_ip']+':30003/v1',service_root=r['root'],full_qualification_root=full['root'])
files={'request-worker.py':(BASE/'xolotl_fullwindow_reuse_load_worker_v20.frozen.py').read_bytes(),'request-plan.json':(json.dumps(plan,indent=2)+'\n').encode()}
record=dict(unix=time.time(),phase='prepared',root=r['root']+'/qualification-fullwindow-load-v41',allocation_uid=r['allocation_uid'],gpu_uuid=r['gpu_uuid'],public_fixture_only=True,files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in files.items()])
tree=ast.parse((BASE/'launch_xolotl_fullwindow_reuse_load_v20.frozen.py').read_text())
bodies=[node.value for node in ast.walk(tree) if isinstance(node,ast.Constant) and isinstance(node.value,str) and 'service=@SERVICE@;prior=@PRIOR@' in node.value]
if len(bodies)!=1:raise RuntimeError('exact full-window load bootstrap differs')
code=bodies[0].replace('@R@',repr(record)).replace('@SERVICE@',repr(r)).replace('@PRIOR@',repr(full)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in files.items()}))
compile(code,'packed-fullwindow-load-v41','exec')
with ip.open('x') as f:f.write(json.dumps(record,indent=2)+'\n')
record['phase']='launch_requested';ip.write_text(json.dumps(record,indent=2)+'\n')
try:
    record['process']=jump(code);record['phase']='launched'
except BaseException:
    record['phase']='launch_outcome_requires_inspection';ip.write_text(json.dumps(record,indent=2)+'\n');raise
ip.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps({'phase':record['phase'],'process':record['process'],'actual_prompt_tokens':1048504,'scope':'Fixed full-window cache hits,16 measured requests each at concurrency1/2; not cold concurrency or long output'}))
