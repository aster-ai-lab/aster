"""Read exact jump-side full-window load result and client identity."""
import json
from xolotl_trial_common import BASE,jump
r=json.loads((BASE/'xolotl-resident-fullwindow-load-v41-intent.json').read_text())
code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact V41 root differs')
p=root/'request-result.json'
if p.exists():
 if p.is_symlink() or p.stat().st_size>262144:raise RuntimeError('bounded full-window load receipt exceeded')
 out['result']=json.loads(p.read_text())
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_client_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
print(json.dumps(out))
'''.replace('@R@',repr(r))
v=jump(code);(BASE/'xolotl-resident-fullwindow-load-v41-latest.json').write_text(json.dumps(v,indent=2)+'\n');q=v.get('result',{})
print(json.dumps({'phase':q.get('phase'),'exact_client_alive':v['exact_client_alive'],'tests':len(q.get('tests',[])),'error':q.get('error'),'levels':q.get('levels',[])},indent=2))
