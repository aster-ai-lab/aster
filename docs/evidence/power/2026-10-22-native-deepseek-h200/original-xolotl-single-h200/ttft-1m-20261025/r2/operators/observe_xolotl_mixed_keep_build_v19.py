import json
from xolotl_trial_common import BASE,observe,owned
r=json.loads((BASE/'xolotl-mixed-keep-build-v19-intent.json').read_text())
code='''import json,time
from pathlib import Path
root=Path(@ROOT@);out=dict(unix=time.time())
for name in ['result.json','process.json']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>1024**2:raise RuntimeError('bounded build record differs')
  out[name]=json.loads(p.read_text())
proof=out.get('process.json')
if proof:
 p=Path('/proc/'+str(proof['pid'])+'/stat')
 out['exact_build_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==proof['start_ticks']
out['logs']={}
for p in root.glob('*.log'):
 if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>4*1024**2:raise RuntimeError('build log bound/owner differs')
 out['logs'][p.name]=p.read_text(errors='replace')[-3500:]
print(json.dumps(out))
'''.replace('@ROOT@',repr(r['root']))
proof=observe()
if proof['job_uid']!=r['allocation_uid']:raise RuntimeError('cache build allocation identity differs')
out=owned(code);(BASE/'xolotl-mixed-keep-build-v19-latest.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
