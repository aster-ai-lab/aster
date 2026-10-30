"""Drain the previous GPU only after in-flight requests finish and the new VIP backend is proven."""
import json,time
from xolotl_serving_v8_frozen_v15 import BASE,observe,owned
ip=BASE/'xolotl-previous-serving-v8-drain-for-v15.json'
if ip.exists():raise RuntimeError('previous serving drain intent exists; inspect it')
r=json.loads((BASE/'xolotl-stable-service-v8-intent.json').read_text());current=json.loads((BASE/'xolotl-stable-service-v15-intent.json').read_text());o=observe()
if o['job_uid']!=r['allocation_uid'] or o['phase']!='Running' or r['gpu_uuid']==current['gpu_uuid']:raise RuntimeError('previous/current distinct serving identities differ')
code='''import json,os,signal,time,urllib.request
from pathlib import Path
r=@R@;current=@CURRENT@
q=json.loads((Path(current['root'])/'service-longctx-v1-result.json').read_text())
if q['phase']!='serving' or q['gpu_uuid']!=current['gpu_uuid']:raise RuntimeError('new server must remain ready')
key=(Path(r['root'])/'credentials/api-key.txt').read_text().strip();request=urllib.request.Request('http://127.0.0.1:30003/slots',headers={'Authorization':'Bearer '+key})
with urllib.request.urlopen(request,timeout=3) as response:slots=json.load(response)
if len(slots)!=1 or slots[0]['is_processing']:raise RuntimeError('previous server still has an in-flight request; retry after it finishes')
p=r['process'];stat=Path('/proc/'+str(p['pid'])+'/stat')
if stat.stat().st_uid!=10250 or int(stat.read_text().split(') ',1)[1].split()[19])!=p['start_ticks']:raise RuntimeError('previous exact supervisor identity changed')
os.kill(p['pid'],signal.SIGTERM);print(json.dumps(dict(unix=time.time(),signal='SIGTERM',exact_process=p,old_slots_idle=True,new_gpu=current['gpu_uuid'])))
'''.replace('@R@',repr(r)).replace('@CURRENT@',repr(current))
compile(code,'old-serving-graceful-role-drain','exec');signal=owned(code);rec=dict(phase='drain_requested',unix=time.time(),signal=signal);ip.write_text(json.dumps(rec,indent=2)+'\n');print(json.dumps(rec,indent=2))
