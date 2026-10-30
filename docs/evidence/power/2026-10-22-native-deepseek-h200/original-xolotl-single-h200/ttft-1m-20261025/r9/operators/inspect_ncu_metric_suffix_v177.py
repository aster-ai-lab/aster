"""Inspect only explicitly named metric suffixes after retaining the overbroad query refusal."""
import json
from xolotl_trial_common import BASE,observe,owned,lease
from aster_ncu_metrics_v171 import METRICS
ip=BASE/'xolotl-ncu-suffix-query-v177.json'
if ip.exists():raise RuntimeError('exact suffix query already has an intent; inspect it')
o=observe()
if o['job_uid']!='7455d989-8255-4f7c-ae8f-3cc0aa5928e4' or o['phase']!='Running':raise RuntimeError('exact optimization ownership differs')
root=lease()['remote']+'/ncu-metric-suffix-v177-cpu'
code='''import hashlib,json,os,subprocess,time
from pathlib import Path
root=Path(@ROOT@);metrics=@METRICS@
if root.exists() or os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250:raise RuntimeError('exclusive exact suffix query root differs')
os.umask(0o077);root.mkdir(mode=0o700)
env={key:os.environ[key] for key in ['PATH','LD_LIBRARY_PATH'] if key in os.environ};env.update(CUDA_VISIBLE_DEVICES='0',LC_ALL='C')
bases=','.join(dict.fromkeys(metric.split('.',1)[0] for metric in metrics))
with (root/'query.stdout.log').open('xb') as stdout,(root/'query.stderr.log').open('xb') as stderr:
 q=subprocess.run(['/usr/local/bin/ncu','--query-metrics-mode','suffix','--metrics',bases,'--csv'],stdout=stdout,stderr=stderr,env=env,timeout=25)
p=root/'query.stdout.log';e=root/'query.stderr.log'
if p.stat().st_size>262144 or e.stat().st_size>65536:raise RuntimeError('explicit metric suffix query exceeded bounded read')
raw=p.read_bytes();err=e.read_text()
result=dict(unix=time.time(),root=str(root),query_returncode=q.returncode,stdout_bytes=len(raw),stdout_sha256=hashlib.sha256(raw).hexdigest(),stdout=raw.decode(),stderr=err,
    inference_sent=False,model_touched=False,profiling_executed=False,hardware_counters_collected=False)
(root/'result.json').write_text(json.dumps(result,indent=2)+'\\n');print(json.dumps(result))
'''.replace('@ROOT@',repr(root)).replace('@METRICS@',repr(METRICS))
compile(code,'ncu-metric-suffix-v177','exec')
with ip.open('x') as output:output.write(json.dumps(dict(root=root,phase='launch_requested'))+'\n')
r=owned(code);ip.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r,indent=2))
