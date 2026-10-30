"""Bounded read of one explicitly public qualification intent; never production text logs."""
import json,sys
from xolotl_trial_common import BASE,jump
name=sys.argv[1]
if '/' in name or '\\' in name or not name.endswith('-intent.json'):raise RuntimeError('one local intent filename required')
r=json.loads((BASE/name).read_text())
if not r.get('public_fixture_only'):raise RuntimeError('public-only intent required')
code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out=dict(unix=time.time(),root=str(root))
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('public client root owner differs')
for name in ['request-result.json','request-process.json']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>1024**2:raise RuntimeError('bounded public client artifact differs')
  out[name]=json.loads(p.read_text())
proof=r.get('process')
if proof:
 p=Path('/proc/'+str(proof['pid'])+'/stat')
 out['exact_client_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==proof['start_ticks']
print(json.dumps(out))
'''.replace('@R@',repr(r))
o=jump(code);out=BASE/name.replace('-intent.json','-latest.json');out.write_text(json.dumps(o,indent=2)+'\n');print(json.dumps(o,indent=2))
