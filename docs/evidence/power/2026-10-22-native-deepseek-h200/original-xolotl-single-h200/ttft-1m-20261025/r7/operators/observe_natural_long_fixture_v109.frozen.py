"""Observe the exact existing CPU tokenizer process; no resubmission or model inference."""
import json
from xolotl_trial_common import BASE,jump

r=json.loads((BASE/'xolotl-natural-long-fixtures-v109-intent.json').read_text())
code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact fixture owner/root differs')
p=root/'prepare-result.json'
if p.exists():
 if p.is_symlink() or p.stat().st_size>262144:raise RuntimeError('bounded CPU fixture receipt differs')
 out['result']=json.loads(p.read_text())
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_cpu_process_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
if out.get('result',{}).get('phase')=='failed':
 p=root/'prepare-worker.log'
 if p.exists() and p.stat().st_size<65536:out['failure_log']=p.read_text()[-3000:]
print(json.dumps(out))
'''.replace('@R@',repr(r))
v=jump(code)
(BASE/'xolotl-natural-long-fixtures-v109-latest.json').write_text(json.dumps(v,indent=2)+'\n')
q=v.get('result',{})
print(json.dumps(dict(phase=q.get('phase'),alive=v['exact_cpu_process_alive'],error=q.get('error'),
    fixtures=[dict(case=f['case'],input_tokens=f['actual_prompt_tokens'],output_budget=f['max_output_tokens']) for f in q.get('fixtures',[])])))
