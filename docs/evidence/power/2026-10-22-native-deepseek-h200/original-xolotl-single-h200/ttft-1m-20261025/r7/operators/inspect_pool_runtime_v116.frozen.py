"""Bounded read-only current GPU/configuration inventory, no extra inference or profiling."""
import json
from xolotl_trial_common import BASE, observe, owned
r=json.loads((BASE/'xolotl-pool-service-v108-pool_on-intent.json').read_text());o=observe()
if o['job_uid']!=r['allocation_uid']:raise RuntimeError('exact isolated profile allocation differs')
code='''import hashlib,json,subprocess
from pathlib import Path
r=@R@;root=Path(r['root'])
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact service inventory owner differs')
config=json.loads((root/'server-config.json').read_text());log=root/'engine.log'
if log.is_symlink() or log.stat().st_size>8*1024**2:raise RuntimeError('bounded exact numeric log differs')
lines=log.read_text(errors='strict').splitlines()
prefixes=('ds41:','ds41_moe:','strata serve:','serve: DSpark','DSpark:','ds41_dense:','resident MMQ stage:','tier:','arena:')
numeric=[s for s in lines[:300] if s.startswith(prefixes)]
result=dict(config=config,engine_env=r['plan']['engine_env'],startup_numeric=numeric,
 gpu=subprocess.run(['nvidia-smi','--query-gpu=uuid,memory.used,memory.total,clocks.sm,clocks.mem,power.limit,pcie.link.gen.current,pcie.link.width.current','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout,
 version=subprocess.run(['/usr/local/cuda/bin/nsys','--version'],capture_output=True,text=True,check=True,timeout=5).stdout)
print(json.dumps(result))
'''.replace('@R@',repr(r))
q=owned(code)
key=(BASE/'access-private-xolotl-9c996c5a434b/api-key.txt').read_bytes().strip()
if key in json.dumps(q).encode():raise RuntimeError('actual credential negative control failed')
with (BASE/'xolotl-pool-runtime-v116.json').open('x') as output:output.write(json.dumps(q,indent=2)+'\n')
print(json.dumps(q,indent=2))
