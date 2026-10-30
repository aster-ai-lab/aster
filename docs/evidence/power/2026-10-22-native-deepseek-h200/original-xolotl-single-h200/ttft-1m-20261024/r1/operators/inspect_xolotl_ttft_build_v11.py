"""Read exact stable host compile/link recipe; no build or GPU mutation."""
import json
from xolotl_trial_common import BASE,owned
r=json.loads((BASE/'xolotl-stable-service-v8-intent.json').read_text())
code='''import hashlib,json,subprocess
from pathlib import Path
r=@R@;binary=Path(r['plan']['binary']);parent=binary.parent.parent
if hashlib.sha256(binary.read_bytes()).hexdigest()!=r['plan']['binary_sha256']:raise RuntimeError('stable binary differs')
p=parent/'source/tools/v41/engine/ds41_generate.cpp';raw=p.read_bytes()
q=subprocess.run(['ninja','-t','commands','ds41_generate'],cwd=parent/'build',capture_output=True,text=True,check=True,timeout=10)
if len(q.stdout)>1024**2:raise RuntimeError('stable recipe bound exceeded')
lines=q.stdout.splitlines();comp=[s for s in lines if ' -c ' in s and str(p) in s];links=[s for s in lines if ' -o ds41_generate ' in s]
if len(comp)!=1 or len(links)!=1:raise RuntimeError('unique stable generate compile/link recipes missing')
v10=Path(@V10@);log=v10/'engine.log';metrics=[]
if log.exists() and log.stat().st_size<8*1024**2:metrics=[s for s in log.read_text(errors='replace').splitlines() if s.startswith(('batch slots:','strata batch:','serve: ready','serve: conversation pool'))][:15]
print(json.dumps(dict(parent=str(parent),source_path=str(p),source_sha256=hashlib.sha256(raw).hexdigest(),source_text=raw.decode(),compile_command=comp[0],link_command=links[0],v10_slot_metrics=metrics)))
'''.replace('@R@',repr(r)).replace('@V10@',repr(json.loads((BASE/'xolotl-ttft-shadow-v10-intent.json').read_text())['root']))
out=owned(code);source=out.pop('source_text');(BASE/'xolotl-ttft-v11-ds41_generate.original.cpp').write_text(source,newline='\n');(BASE/'xolotl-ttft-v11-build-recipe.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out,indent=2))
