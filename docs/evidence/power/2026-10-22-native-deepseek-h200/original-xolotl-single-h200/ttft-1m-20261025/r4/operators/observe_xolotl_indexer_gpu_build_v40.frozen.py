"""Inspect the exact controlled CUDA build, with no model restart on poll expiry."""
import json
from xolotl_trial_common import BASE,observe,owned
r=json.loads((BASE/'xolotl-indexer-gpu-build-v40-intent.json').read_text());o=observe()
if o['job_uid']!=r['allocation_uid']:raise RuntimeError('exact V40 allocation differs')
code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()}
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact CUDA build root differs')
p=root/'gpu-build-result.json'
if p.exists():
 if p.is_symlink() or p.stat().st_size>262144:raise RuntimeError('CUDA build receipt bound exceeded')
 out['result']=json.loads(p.read_text())
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_build_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
for name in ['exact-mask-cuda-compile.stderr.log','exact-mask-probe-compile.stderr.log','actual-graph-driver-compile.stderr.log','candidate-model-link.stderr.log','gpu-build-worker.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>2*1024**2:raise RuntimeError('bounded compiler log exceeded')
  text=p.read_text(errors='replace');out[name]=text[:2000]+('\\n[...middle omitted...]\\n' if len(text)>5000 else '')+(text[-3000:] if len(text)>2000 else '')
print(json.dumps(out))
'''.replace('@R@',repr(r))
v=owned(code);(BASE/'xolotl-indexer-gpu-build-v40-latest.json').write_text(json.dumps(v,indent=2)+'\n');print(json.dumps(v,indent=2))
