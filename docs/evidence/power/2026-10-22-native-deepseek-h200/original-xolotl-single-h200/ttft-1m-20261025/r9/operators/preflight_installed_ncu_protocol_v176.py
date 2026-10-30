"""Verify the installed CPU pipe and H200 metric query without profiling or touching a model."""
import json
from xolotl_trial_common import BASE,observe,owned,lease
from aster_ncu_metrics_v171 import METRICS

ip=BASE/'xolotl-ncu-preflight-v176.json'
if ip.exists():raise RuntimeError('exact installed profiler preflight already has a receipt; inspect it')
o=observe()
if o['phase']!='Running' or o['job_uid']!='7455d989-8255-4f7c-ae8f-3cc0aa5928e4':raise RuntimeError('exact independent optimization allocation differs')
root=lease()['remote']+'/ncu-preflight-v176-cpu'
code='''import hashlib,json,os,subprocess,time,sys
from pathlib import Path
root=Path(@ROOT@);metrics=@METRICS@
if os.getuid()!=10250 or root.exists() or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250:raise RuntimeError('exclusive exact CPU profiler preflight root differs')
os.umask(0o077);root.mkdir(mode=0o700)
env={key:os.environ[key] for key in ['PATH','LD_LIBRARY_PATH'] if key in os.environ};env.update(CUDA_VISIBLE_DEVICES='',LC_ALL='C')
ncu='/usr/local/bin/ncu'
cpu_code="import sys;print('READY_CPU',flush=True);line=sys.stdin.readline();print('ECHO_CPU '+line.strip(),flush=True)"
smoke=subprocess.run([ncu,'--profile-from-start','off','--clock-control','none','--cache-control','none','--log-file',str(root/'cpu-protocol-ncu.log'),sys.executable,'-u','-c',cpu_code],input='public-profile-pipe-check\\n',capture_output=True,text=True,env=env,timeout=25)
for label,text in [('cpu-protocol.stdout.log',smoke.stdout),('cpu-protocol.stderr.log',smoke.stderr)]:
 if len(text)>262144:raise RuntimeError('bounded CPU profiler smoke output exceeded')
 (root/label).write_text(text)
result=dict(unix=time.time(),phase='preflight',root=str(root),allocation_uid=@UID@,
    GPU_math_executed=False,model_touched=False,profiling_executed=False,hardware_counters_collected=False,
    clock_control='none',cache_control='none',smoke_returncode=smoke.returncode,
    smoke_stdout_exact=smoke.stdout=='READY_CPU\\nECHO_CPU public-profile-pipe-check\\n')
try:
 if smoke.returncode or not result['smoke_stdout_exact']:raise RuntimeError('installed ncu bidirectional CPU pipe failed')
 bases=','.join(dict.fromkeys(value.split('.',1)[0] for value in metrics))
 query=subprocess.run([ncu,'--query-metrics-mode','all','--metrics',bases],capture_output=True,text=True,env=dict(env,CUDA_VISIBLE_DEVICES='0'),timeout=25)
 if len(query.stdout)>1048576 or len(query.stderr)>262144:raise RuntimeError('bounded installed H200 metric query exceeded')
 (root/'metric-query.stdout.log').write_text(query.stdout);(root/'metric-query.stderr.log').write_text(query.stderr)
 result.update(metric_query_returncode=query.returncode,metric_query_stdout_sha256=hashlib.sha256(query.stdout.encode()).hexdigest(),
    confirmed_metrics=[value for value in metrics if value in query.stdout],missing_metrics=[value for value in metrics if value not in query.stdout])
 if query.returncode or result['missing_metrics']:raise RuntimeError('installed actual H200 query did not confirm exact requested metrics')
 result.update(phase='passed',finished_unix=time.time())
except BaseException as error:result.update(phase='failed',error=type(error).__name__+': '+str(error)[:2000],finished_unix=time.time())
(root/'result.json').write_text(json.dumps(result,indent=2)+'\\n');print(json.dumps(result))
'''.replace('@ROOT@',repr(root)).replace('@METRICS@',repr(METRICS)).replace('@UID@',repr(o['job_uid']))
compile(code,'exact-installed-ncu-preflight-v176','exec')
with ip.open('x') as output:output.write(json.dumps(dict(phase='launch_requested',root=root,allocation_uid=o['job_uid']))+'\n')
result=owned(code)
ip.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
if result['phase']!='passed':raise RuntimeError(result['error'])
