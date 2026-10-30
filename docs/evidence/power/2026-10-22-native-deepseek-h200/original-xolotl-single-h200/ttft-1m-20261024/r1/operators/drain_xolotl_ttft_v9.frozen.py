"""Drain only the completed public baseline on the reserved optimization GPU."""
import json,time
from xolotl_trial_common import BASE,observe,owned
ip=BASE/'xolotl-ttft-v9-drain.json'
if ip.exists():raise RuntimeError('drain record exists; observe exact service')
r=json.loads((BASE/'xolotl-ttft-shadow-v9-intent.json').read_text());o=observe()
if o['job_uid']!=r['allocation_uid']:raise RuntimeError('optimization identity differs')
code='''import json,os,signal,time,urllib.request
from pathlib import Path
r=@R@;root=Path(r['root'])
q=json.loads((root/'qualification-ttft-v9/request-result.json').read_text())
if q['phase']!='passed':raise RuntimeError('public baseline not completed')
key=(root/'credentials/api-key.txt').read_text().strip()
req=urllib.request.Request('http://127.0.0.1:30003/slots',headers={'Authorization':'Bearer '+key})
with urllib.request.urlopen(req,timeout=3) as resp:slots=json.load(resp)
if any(s['is_processing'] for s in slots):raise RuntimeError('public client still active')
p=r['process'];s=Path('/proc/'+str(p['pid'])+'/stat')
if s.stat().st_uid!=10250 or int(s.read_text().split(') ',1)[1].split()[19])!=p['start_ticks']:raise RuntimeError('exact baseline supervisor differs')
os.kill(p['pid'],signal.SIGTERM)
print(json.dumps(dict(unix=time.time(),signal='SIGTERM',exact_process=p,qualification_complete=True)))
'''.replace('@R@',repr(r))
q=owned(code);ip.write_text(json.dumps(dict(phase='drain_requested',signal=q),indent=2)+'\n');print(json.dumps(q))
