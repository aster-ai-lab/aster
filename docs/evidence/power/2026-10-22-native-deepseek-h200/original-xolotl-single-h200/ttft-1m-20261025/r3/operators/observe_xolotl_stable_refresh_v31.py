"""Read the exact isolated candidate state and numeric-only cache diagnostics."""
import json
from xolotl_trial_common import BASE, observe, owned

r = json.loads((BASE / 'xolotl-stable-refresh-v31-intent.json').read_text())
observation = observe()
if observation['job_uid'] != r['allocation_uid']:
    raise RuntimeError('V23 optimization allocation identity differs')
code = '''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out=dict(unix=time.time(),root=str(root))
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('isolated V23 source root identity differs')
for name in ['service-longctx-v1-result.json','service-process.json']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>65536:raise RuntimeError('bounded V23 service receipt differs')
  out[name]=json.loads(p.read_text())
proof=r.get('process')
if proof:
 p=Path('/proc/'+str(proof['pid'])+'/stat')
 out['exact_supervisor_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==proof['start_ticks']
log=root/'engine.log'
if log.exists():
 if log.is_symlink() or log.stat().st_uid!=10250 or log.stat().st_size>8*1024**2:raise RuntimeError('bounded public candidate numeric log differs')
 prefixes=('serve: keep-cache:','serve: cache kept for ','serve: cache regrown:','[keep-chk]','dense prof ','serve: prompt phases: ','serve: prompt ')
 out['numeric_diagnostics']=[line for line in log.read_text(errors='replace').splitlines() if line.startswith(prefixes)][-20:]
print(json.dumps(out))
'''.replace('@R@', repr(r))
result = owned(code)
(BASE / 'xolotl-stable-refresh-v31-latest.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
