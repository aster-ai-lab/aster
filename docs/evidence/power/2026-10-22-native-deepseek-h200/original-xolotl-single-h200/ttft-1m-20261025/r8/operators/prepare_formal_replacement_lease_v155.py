"""Replace only proven-terminal C9 capacity; preserve the active C10 optimization allocation."""
import ast
import json
from pathlib import Path
from xolotl_trial_common import BASE

old = json.loads((BASE/'xolotl-stable-service-v97-intent.json').read_text())
opt = json.loads((BASE/'xolotl-phase-simd-v139-control_b-intent.json').read_text())
guard = '''import json,shlex,subprocess,time
old=@OLD@;opt=@OPT@
def get(kind,name):
 p=subprocess.run(['bash','-lc','. /etc/profile.d/ssh-init.sh >/dev/null 2>&1; exec '+shlex.join(['brainctl','get',kind,name,'-n','ailab-hrprdduanyan','-o','json'])],capture_output=True,text=True,check=True,timeout=15)
 if len(p.stdout)>1048576:raise RuntimeError('bounded exact role metadata exceeded')
 return json.loads(p.stdout)
proof={}
for role,r in [('terminal_formal',old),('independent_optimization',opt)]:
 row={}
 for kind,name,uid in [('rjob',r['allocation_job'],r['allocation_uid']),('replica',r['replica'],r['replica_uid'])]:
  v=get(kind,name)
  if v['metadata']['uid']!=uid:raise RuntimeError('exact bounded role UID differs')
  phase=v['status']['phase']
  if role=='terminal_formal' and phase not in ['Stopped','Failed','Succeeded']:raise RuntimeError('old formal allocation still live or uncertain; do not allocate')
  if role=='independent_optimization' and phase!='Running':raise RuntimeError('optimization state changed; inspect exact handle')
  row[kind]=dict(uid=uid,phase=phase)
 proof[role]=row
s=get('service','aster-xolotl-api-v3pool-20261024')
if s['metadata']['uid']!='1f968a8d-4ddc-4cee-b32a-390cd44210c7' or s['metadata']['annotations']['aster.serving/allocation-uid']!=old['allocation_uid'] or s['spec']['selector']!={'rjob.brainpp.cn/rjob-name':old['allocation_job']}:raise RuntimeError('existing terminal formal VIP ownership differs')
proof.update(unix=time.time(),maximum_authorized_h200=2,new_role='formal-replacement',optimization_changed=False)
print(json.dumps(proof))
'''.replace('@OLD@',repr({k:old[k] for k in ['allocation_job','allocation_uid','replica','replica_uid']})).replace('@OPT@',repr({k:opt[k] for k in ['allocation_job','allocation_uid','replica','replica_uid']}))
source = (BASE/'submit_xolotl_single1m_cont10_lease_v100.frozen.py').read_text()
start = source.index("promotion=json.loads((scratch/'xolotl-stable-recovery-v97-promotion.json').read_text())")
end = source.index("record_path=scratch/",start)
prefix = '''from xolotl_trial_common import jump
GUARD=@GUARD@
previous_terminal=jump(GUARD)
'''.replace('@GUARD@',repr(guard))
source = source[:start]+prefix+'\n'+source[end:]
source = source.replace('xolotl-single1m-cont10-lease-pending.json','xolotl-single1m-cont11-lease-pending.json').replace('aster-xolotl-1mc10-','aster-xolotl-1mc11-')
before = 'bounded six-hour single-H2001M XQ optimization: isolate short-input concurrent/draft/cache behavior and GPU-host execution, then actual1M natural/long-output gates; formalC9 unchanged, maximum two H200'
if source.count(before) != 1:
    raise RuntimeError('exact allocation-purpose anchor differs')
source = source.replace(before, 'bounded six-hour formal1M single-H200 replacement for proven-terminal C9; preserve C10 optimization and same VIP/key; maximum two H200')
anchor = "record['phase']='submit_requested';save()"
if source.count(anchor) != 1:
    raise RuntimeError('unique actual submission boundary differs')
source = source.replace(anchor,"record['pre_submit_guard']=remote_python(GUARD)\n"+anchor)
ast.parse(source)
path = BASE/'submit_xolotl_single1m_cont11_lease_v155.frozen.py'
with path.open('x') as output:
    output.write(source)
print(json.dumps(dict(phase='prepared',formal_replacement=True,terminal_predecessor=old['allocation_uid'],independent_optimization=opt['allocation_uid'],maximum_authorized_h200=2)))
