"""Bounded runtime identities and numeric-only engine phase diagnostics."""
import json, time
from xolotl_trial_common import BASE
import xolotl_trial_common as trial
import xolotl_serving_common as serving

out = dict(unix=time.time(), schema='aster-xolotl-ttft-observation-v9')
for name, transport in [('serving', serving), ('optimization', trial)]:
    o = transport.observe()
    out[name] = dict(job=o['job'], uid=o['job_uid'], phase=o['phase'],
                     gpu=o['resource-result.json']['gpus'][0]['uuid'])
    if name == 'serving':
        intent = json.loads((BASE/'xolotl-stable-service-v8-intent.json').read_text())
        code = '''import json, subprocess
from pathlib import Path
r=@R@; root=Path(r['root'])
if root.is_symlink() or root.stat().st_uid!=10250:raise RuntimeError('runtime root owner differs')
p=root/'engine.log'
if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>16*1024**2:raise RuntimeError('numeric diagnostic bound differs')
prefixes=('serve: prompt phases: ', 'serve: prompt ', 'serve: pass split ', 'serve: VRAM ready seed ', 'strata serve: conversation cache: ', 'serve: reuse ', 'arena rebalance: ')
rows=[s for s in p.read_text(errors='replace').splitlines() if s.startswith(prefixes)][-85:]
print(json.dumps(dict(state=json.loads((root/'service-longctx-v1-result.json').read_text()),config=json.loads((root/'server-config.json').read_text()),numeric_engine_metrics=rows)))
'''.replace('@R@', repr(intent))
    else:
        code = '''import json,subprocess
from pathlib import Path
r=@R@;root=Path(r['remote'])
p=root/'serve-prefill-cache-v4/service-longctx-v1-result.json'
gpu=subprocess.run(['nvidia-smi','--query-compute-apps=pid,gpu_uuid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=8).stdout.strip()
print(json.dumps(dict(old_state=json.loads(p.read_text()),active_gpu_processes=gpu)))
'''.replace('@R@', repr(trial.lease()))
    out[name]['runtime'] = transport.owned(code)
(BASE/'xolotl-ttft-v9-initial-observation.json').write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps(out,indent=2))
