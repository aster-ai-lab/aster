"""Retain the debug-link failure and derive the previously working release-only CPU flags."""
import ast
import hashlib
import json
from xolotl_trial_common import BASE,jump,lease
source=BASE/'inspect_exact_model_geometry_v223.py';raw=source.read_text()
root=lease()['remote']+'/geometry-topk-v223-cpu'
proof=jump('from pathlib import Path;import hashlib,json;p=Path('+repr(root+'/compile.log')+');raw=p.read_bytes();assert len(raw)<65536;print(json.dumps(dict(root='+repr(root)+',compile_log=raw.decode(),compile_log_sha256=hashlib.sha256(raw).hexdigest(),GPU_executed=False)))')
with (BASE/'xolotl-topk-geometry-v223-failure.json').open('x') as out:out.write(json.dumps(proof,indent=2)+'\n')
before="'/usr/bin/c++','-O2','-std=c++17'"
if raw.count(before)!=1:raise RuntimeError('exact metadata compile flag anchor differs')
raw=raw.replace(before,"'/usr/bin/c++','-O3','-DNDEBUG','-std=c++20'").replace('geometry-v223.json','geometry-v223r1.json').replace('geometry-topk-v223-cpu','geometry-topk-v223r1-cpu')
ast.parse(raw)
with (BASE/'inspect_exact_model_geometry_v223r1.py').open('x') as out:out.write(raw)
print(json.dumps(dict(prepared=True,parent_failure_retained=True,new_operator='inspect_exact_model_geometry_v223r1.py',GPU_work_started=False)))
