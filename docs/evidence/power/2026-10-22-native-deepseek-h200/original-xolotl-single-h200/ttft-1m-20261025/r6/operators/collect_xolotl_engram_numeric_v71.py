"""Read bounded public numeric model diagnostics for exact comparison profiles."""
import json
import time
from xolotl_trial_common import BASE, jump, observe, owned

records=[]
for profile in ['gpfs_a','local','gpfs_b']:
    path=BASE/('xolotl-engram-disk-v68-'+profile+'-intent.json')
    if path.exists():
        value=json.loads(path.read_text())
        records.append(dict(profile=profile,root=value['root']))
path=BASE/'xolotl-engram-lut-v75-lut_local-intent.json'
if path.exists():
    value=json.loads(path.read_text())
    records.append(dict(profile='lut_local',root=value['root']))
path=BASE/'xolotl-engram-local-repeat-v79-local_repeat-intent.json'
if path.exists():
    value=json.loads(path.read_text())
    records.append(dict(profile='local_repeat',root=value['root']))
observe()
code='''import json,subprocess,time
from pathlib import Path
rows=@ROWS@;out={'unix':time.time(),'profiles':[]}
prefixes=('serve: keep-cache:','serve: cache kept for ','serve: cache regrown:','[keep-chk]','dense prof ','serve: prompt phases: ','serve: prompt ','chunk prof','resident MMQ stage:','engram decode reads:')
for row in rows:
 root=Path(row['root']);p=root/'engine.log'
 if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact diagnostic directory owner differs')
 if not p.exists():
  out['profiles'].append({'profile':row['profile'],'lines':[],'engine_log_not_created_yet':True});continue
 if p.is_symlink() or p.stat().st_size>8*1024**2:raise RuntimeError('bounded exact model diagnostic exceeded')
 lines=p.read_text(errors='replace').splitlines();selected=[{'line':i+1,'text':line} for i,line in enumerate(lines) if line.startswith(prefixes)]
 if len(selected)>8192:raise RuntimeError('bounded numeric diagnostic count exceeded')
 out['profiles'].append({'profile':row['profile'],'lines':selected})
out['gpu_snapshot']=subprocess.run(['nvidia-smi','--query-gpu=uuid,memory.used,memory.free,memory.total,power.draw,clocks.sm,clocks.mem,utilization.gpu,utilization.memory','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
print(json.dumps(out))
'''.replace('@ROWS@',repr(records))
result=owned(code)
(BASE/'xolotl-engram-comparison-v71-numeric.json').write_text(json.dumps(result,indent=2)+'\n')
watch=json.loads((BASE/'xolotl-readiness-watch-v65-intent.json').read_text())
value=jump('''import json,time
from pathlib import Path
r=@R@;p=Path('/proc/'+str(r['process']['pid'])+'/stat');root=Path(r['root']);q=root/'watch-result.json'
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
if q.is_symlink() or q.stat().st_size>1048576:raise RuntimeError('bounded formal readiness result exceeded')
print(json.dumps({'unix':time.time(),'exact_observer_alive':alive,'result':json.loads(q.read_text())}))
'''.replace('@R@',repr(watch)))
(BASE/'xolotl-readiness-watch-v65-latest.json').write_text(json.dumps(value,indent=2)+'\n')
print(json.dumps(dict(profiles=[dict(profile=row['profile'],lines=len(row['lines'])) for row in result['profiles']],formal_observer_alive=value['exact_observer_alive'],formal_result=value['result']),indent=2))
