"""Preserve V112, resume its exact client/model after a local Windows replace refusal."""
import hashlib
import json
import time
from xolotl_trial_common import BASE

source = BASE / 'run_pool_seed_comparison_v112.frozen.py'
old = json.loads((BASE / 'xolotl-pool-seed-v112-controller.json').read_text())
if old['phase'] != 'requires_inspection' or 'WinError 5' not in old['error']:
    raise RuntimeError('exact local observer interruption must be retained')
if old['profile'] != 'pool_off' or old['profiles']:
    raise RuntimeError('resume only the exact first existing comparison profile')
raw = source.read_text()

def replace(before, after):
    global raw
    if raw.count(before) != 1:
        raise RuntimeError('pinned controller anchor differs')
    raw = raw.replace(before, after)

replace("PATH=BASE/'xolotl-pool-seed-v112-controller.json'",
        "PATH=BASE/'xolotl-pool-seed-v112r1-controller.json'")
replace("           actual1m_input_tested=False,goal_complete=False,performance_300_qualified=False)",
        "           actual1m_input_tested=False,goal_complete=False,performance_300_qualified=False,\n"
        "           resumes='xolotl-pool-seed-v112-controller.json',\n"
        "           resume_reason='Retained local Windows atomic-replace refusal; no GPU/client failure or restart')")
replace("    STATE['observed_unix']=time.time();p=PATH.with_suffix('.tmp');p.write_text(json.dumps(STATE,indent=2)+'\\n');p.replace(PATH)",
        "    STATE['observed_unix']=time.time();p=PATH.with_suffix('.tmp');p.write_text(json.dumps(STATE,indent=2)+'\\n')\n"
        "    for attempt in range(12):\n"
        "        try:\n"
        "            p.replace(PATH);return\n"
        "        except PermissionError:\n"
        "            if attempt==11:raise\n"
        "            time.sleep(0.1*(attempt+1))")
replace("        run('manage_pool_long_v111.frozen.py',profile,'launch')",
        "        if not (BASE/('xolotl-pool-long-v111-'+profile+'-intent.json')).exists():\n"
        "            run('manage_pool_long_v111.frozen.py',profile,'launch')")
compile(raw, 'run_pool_seed_comparison_v112r1.frozen.py', 'exec')
destination=BASE/'run_pool_seed_comparison_v112r1.frozen.py'
with destination.open('x',encoding='utf-8',newline='\n') as output:
    output.write(raw)
receipt=dict(unix=time.time(),parent=source.name,parent_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
             resumed=destination.name,resumed_sha256=hashlib.sha256(destination.read_bytes()).hexdigest(),
             model_restarted=False,existing_client_relaunched=False,acceptance_gates_unchanged=True,
             previous_observer_error=old['error'])
with (BASE/'xolotl-pool-controller-resume-v112r1.json').open('x') as output:
    output.write(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt))
