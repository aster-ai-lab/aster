"""Stop one exact idle optimization allocation after its public successor is proven."""
import json, time
from xolotl_trial_common import BASE, observe, owned, jump

ip = BASE / 'xolotl-v8-resource-stop-for-1m-cont6.json'
if ip.exists():
    raise RuntimeError('resource stop intent exists; inspect exact job instead of repeating stop')
old = json.loads((BASE / 'xolotl-stable-service-v8-intent.json').read_text())
new = json.loads((BASE / 'xolotl-stable-service-v15-intent.json').read_text())
promotion = json.loads((BASE / 'xolotl-ttft-v15-promotion.json').read_text())
gate = json.loads((BASE / 'xolotl-mixed-cache-gpu-gate-v22-run-intent.json').read_text())
observation = observe()
if observation['job_uid'] != old['allocation_uid'] or promotion['phase'] != 'promoted' or gate['phase'] != 'passed':
    raise RuntimeError('idle former server, proven public successor and completed byte gate are required')
code = '''import json,subprocess,time
from pathlib import Path
old=@OLD@;state=json.loads((Path(old['root'])/'service-longctx-v1-result.json').read_text())
active=subprocess.run(['nvidia-smi','--query-compute-apps=pid,gpu_uuid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=8).stdout.strip()
if state['phase']!='stopped' or not state.get('api_reaped') or active:raise RuntimeError('exact old optimization GPU must be idle')
print(json.dumps(dict(unix=time.time(),old_api_reaped=True,gpu_apps=active)))
'''.replace('@OLD@', repr(old))
idle = owned(code)
record = dict(unix=time.time(), phase='prepared', old_allocation_uid=old['allocation_uid'], old_replica_uid=old['replica_uid'],
              serving_allocation_uid=new['allocation_uid'], old_gpu_idle_proof=idle, maximum_h200=2)
with ip.open('x') as stream:
    stream.write(json.dumps(record, indent=2) + '\n')
code = '''import json,shlex,subprocess,time,urllib.request
from pathlib import Path
old=@OLD@;new=@NEW@
def cli(args):
 p=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(['brainctl']+args)],capture_output=True,text=True,check=True,timeout=15)
 if len(p.stdout)>1048576:raise RuntimeError('bounded stop metadata exceeded')
 return p
for kind,name,uid in [('rjob',old['allocation_job'],old['allocation_uid']),('replica',old['replica'],old['replica_uid']),('rjob',new['allocation_job'],new['allocation_uid']),('replica',new['replica'],new['replica_uid'])]:
 item=json.loads(cli(['get',kind,name,'-n','ailab-hrprdduanyan','-o','json']).stdout)
 if item['metadata']['uid']!=uid or item['status']['phase']!='Running':raise RuntimeError('fresh exact old/new resource identity differs')
service=json.loads(cli(['get','service','aster-xolotl-api-v3pool-20261024','-n','ailab-hrprdduanyan','-o','json']).stdout)
if service['metadata']['uid']!='1f968a8d-4ddc-4cee-b32a-390cd44210c7' or service['metadata']['annotations']['aster.serving/allocation-uid']!=new['allocation_uid'] or service['spec']['selector']!={'rjob.brainpp.cn/rjob-name':new['allocation_job']}:raise RuntimeError('public VIP must belong to the live successor')
key=(Path(new['root'])/'credentials/api-key.txt').read_text().strip();request=urllib.request.Request('http://10.102.93.68/health',headers={'Authorization':'Bearer '+key})
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request,timeout=5) as response:health=json.load(response)
if not health.get('loaded') or health.get('max_context')!=1048576:raise RuntimeError('new public 1M service must remain ready')
stopped=cli(['stop','rjob/'+old['allocation_job'],'-n','ailab-hrprdduanyan'])
print(json.dumps(dict(unix=time.time(),stop_cli_exit=stopped.returncode,stopped_name=old['allocation_job'],stopped_uid=old['allocation_uid'],new_public_service_preserved=True)))
'''.replace('@OLD@', repr(old)).replace('@NEW@', repr(new))
record['phase'] = 'stop_requested'
ip.write_text(json.dumps(record, indent=2) + '\n')
record['result'] = jump(code, 80)
record['phase'] = 'stop_command_returned'
ip.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record, indent=2))
