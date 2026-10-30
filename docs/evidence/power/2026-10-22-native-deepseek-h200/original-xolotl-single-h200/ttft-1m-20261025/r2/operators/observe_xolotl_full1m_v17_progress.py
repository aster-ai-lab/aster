"""Exact model-process and GPU observation for the isolated public 1M test."""
import json
from xolotl_trial_common import BASE, observe, owned

r = json.loads((BASE / 'xolotl-ttft-shadow-v15-intent.json').read_text())
observation = observe()
if observation['job_uid'] != r['allocation_uid']:
    raise RuntimeError('exact optimization allocation differs')
code = '''import json,subprocess,time
from pathlib import Path
root=Path(@ROOT@);q=json.loads((root/'service-longctx-v1-result.json').read_text());proc=q['api_process'];p=Path('/proc/'+str(proc['pid'])+'/stat')
api_alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==proc['start_ticks']
gpu=subprocess.run(['nvidia-smi','--query-gpu=uuid,utilization.gpu,memory.used,memory.total','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=8).stdout.strip()
log=root/'api-longctx-v1-attempt1.stdout.log'
if log.is_symlink() or log.stat().st_uid!=10250 or log.stat().st_size>4*1024**2:raise RuntimeError('isolated public API log identity/bound differs')
with log.open('rb') as f:f.seek(max(0,log.stat().st_size-32768));tail=f.read(32768).decode('utf-8',errors='replace')
progress=[line for line in tail.splitlines() if line.startswith(('[strata] reading the prompt:','[strata] done:','[strata] the engine reported an error:'))][-6:]
print(json.dumps(dict(unix=time.time(),api_exact_process_alive=api_alive,service_phase=q['phase'],gpu_csv=gpu,public_numeric_progress=progress)))
'''.replace('@ROOT@', repr(r['root']))
result = owned(code)
(BASE / 'xolotl-full1m-v17-progress-latest.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
