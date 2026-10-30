"""Recover only the exact frozen formal native process after retained inactivity proof."""
import json,time
from xolotl_serving_common import BASE,observe,owned
path=BASE/'xolotl-frozen-native-recovery-v412.json'
if path.exists():raise RuntimeError('Recovery intent exists; inspect instead of repeating signal')
r=json.loads((BASE/'xolotl-stable-service-v368-intent.json').read_text());o=observe()
if o['job_uid']!=r['allocation_uid']:raise RuntimeError('Exact C14 required')
samples=[]
for p in BASE.glob('xolotl-wait-metrics-v409-*.json'):
 v=json.loads(p.read_text());n=v.get('native',{})
 if n.get('pid')==316 and v.get('GPU_csv','').split(',')[1:2]==[' 0']:samples.append(v)
if not samples:raise RuntimeError('No retained zero-GPU native sample')
old=min(samples,key=lambda v:v['unix'])
if time.time()-old['unix']<120:raise RuntimeError('At least 120 seconds of retained inactivity evidence required')
record=dict(unix=time.time(),phase='recovery_requested',reason='WorkBuddy fresh HI hangs; retained native inactivity while API reports reading',old_observation_unix=old['unix'],old_native=old['native'],formal_allocation_uid=r['allocation_uid'],model_config_changed=False,optimization_resumed=False)
with path.open('x') as f:f.write(json.dumps(record,indent=2)+'\n')
code='''import json,os,signal,subprocess,time,urllib.request
from pathlib import Path
r=@R@;old=@OLD@;root=Path(r['root']);row=r['runtime_native'];p=Path('/proc/'+str(row['pid']));s=(p/'stat').read_text().split(') ',1)[1].split()
if p.stat().st_uid!=10250 or int(s[19])!=row['start_ticks'] or int(s[1])!=314 or os.readlink(p/'exe')!=r['plan']['binary']:raise RuntimeError('Exact formal native identity differs')
ticks=int(s[11])+int(s[12]);io={k:int(v) for k,v in (line.split(':',1) for line in (p/'io').read_text().splitlines())}
if (ticks-old['native']['cpu_ticks'])/os.sysconf('SC_CLK_TCK')>0.5 or io!=old['native']['io']:raise RuntimeError('Native made CPU/IO progress; refuse frozen-process recovery')
gpu=subprocess.run(['nvidia-smi','--query-gpu=uuid,utilization.gpu','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
if gpu!=r['gpu_uuid']+', 0':raise RuntimeError('Physical GPU is busy or changed; inspect before recovery')
key=(root/'credentials/api-key.txt').read_text().strip();req=urllib.request.Request('http://127.0.0.1:30003/metrics',headers={'Authorization':'Bearer '+key,'Accept':'application/json'})
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(req,timeout=5) as response:
 raw=response.read(1048577)
 if len(raw)>1048576:raise RuntimeError('Metrics bound exceeded')
 live=json.loads(raw)['live']
if live.get('state')!='reading' or live.get('prompt_tokens')!=672888 or live.get('generated')!=0:raise RuntimeError('Exact frozen old request changed; inspect')
os.kill(row['pid'],signal.SIGTERM)
print(json.dumps(dict(unix=time.time(),signal='SIGTERM',native_PID=row['pid'],start_ticks=row['start_ticks'],API_PID=314,API_not_signalled=True,VIP_unchanged=True,native_cpu_seconds_since_retained_observation=(ticks-old['native']['cpu_ticks'])/os.sysconf('SC_CLK_TCK'),observed_stall_seconds=time.time()-old['unix'],GPU_utilization_percent=0)))
'''.replace('@R@',repr(r)).replace('@OLD@',repr(old))
try:record.update(result=owned(code,65),phase='native_recovery_signal_sent')
except BaseException as e:
 record.update(phase='recovery_outcome_requires_inspection',error_type=type(e).__name__);path.write_text(json.dumps(record,indent=2)+'\n');raise
path.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record,indent=2))
