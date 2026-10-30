"""Read clock/power/temperature/counters and bounded exact phase log; never change GPU policy."""
import json
import time
from xolotl_trial_common import BASE, observe, owned

ip = BASE/'xolotl-h200-capacity-v154.json'
if ip.exists():
    raise RuntimeError('exact numeric capacity observation is already frozen')
r = json.loads((BASE/'xolotl-phase-simd-v139-control_b-intent.json').read_text())
o = observe()
if o['job_uid'] != r['allocation_uid']:
    raise RuntimeError('exact current optimization allocation differs')
code = '''import hashlib,json,shutil,subprocess,time,xml.etree.ElementTree as ET
from pathlib import Path
r=@R@
p=subprocess.run(['nvidia-smi','-q','-x'],capture_output=True,text=True,check=True,timeout=8)
if len(p.stdout)>262144:raise RuntimeError('bounded numeric GPU XML exceeded')
tree=ET.fromstring(p.stdout);gpus=tree.findall('gpu')
if len(gpus)!=1 or gpus[0].findtext('uuid')!=r['gpu_uuid']:raise RuntimeError('exact independent H200 UUID differs')
gpu=gpus[0]
def value(node):
 return {child.tag:value(child) for child in node} if len(node) else node.text
sections=['product_name','uuid','performance','gpu_performance_state','clocks','applications_clocks','default_applications_clocks','max_clocks','power_readings','gpu_power_readings','temperature','utilization','fb_memory_usage','compute_mode','clocks_event_reasons','clocks_event_reasons_counters']
out=dict(unix=time.time(),gpu={name:value(gpu.find(name)) for name in sections if gpu.find(name) is not None},gpu_policy_changed=False,
 profiling_tools={name:shutil.which(name) for name in ['ncu','nsys','perf']},inference_sent=False)
root=Path(r['root']);p=root/'engine.log'
if root.resolve()!=root or root.stat().st_uid!=10250 or p.is_symlink() or p.stat().st_size>8*1024**2:raise RuntimeError('bounded exact phase log differs')
raw=p.read_bytes();lines=raw.decode(errors='replace').splitlines()
out['phase_log_snapshot']=dict(bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),
 phases=[line for line in lines if line.startswith('aster prefill phase:')][-48:],
 completed_prompt_phases=[line for line in lines if line.startswith('serve: prompt phases:')][-4:])
print(json.dumps(out))
'''.replace('@R@',repr(r))
result = owned(code)
with ip.open('x') as output:
    output.write(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
