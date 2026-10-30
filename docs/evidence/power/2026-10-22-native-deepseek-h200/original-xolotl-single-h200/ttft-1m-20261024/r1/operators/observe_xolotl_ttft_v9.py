"""Observe the exact V9 public-only client and isolated service, without re-launch."""
import json,time
from xolotl_trial_common import BASE,jump
r=json.loads((BASE/'xolotl-ttft-qualification-v9-intent.json').read_text())
code='''import json,os,time
from pathlib import Path
r=@R@;root=Path(r['root']);out=dict(unix=time.time())
for name in ['request-result.json','request-process.json']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>1024**2:raise RuntimeError('public client artifact bound/owner differs')
  out[name]=json.loads(p.read_text())
out['service']=json.loads((root.parent/'service-longctx-v1-result.json').read_text())
p=root.parent/'engine.log'
if p.stat().st_size>8*1024**2:raise RuntimeError('public-only engine log bound differs')
out['numeric_phases']=[s for s in p.read_text(errors='replace').splitlines() if s.startswith(('serve: prompt phases: ','serve: prompt ','strata serve: conversation cache: ','strata batch: '))][-12:]
print(json.dumps(out))
'''.replace('@R@',repr(r))
out=jump(code);(BASE/'xolotl-ttft-qualification-v9-latest.json').write_text(json.dumps(out,indent=2)+'\n')
q=out.get('request-result.json',{});print(json.dumps(dict(phase=q.get('phase'),error=q.get('error'),summary=q.get('short_summary'),tests=[dict(name=x['name'],ttft=round(x['ttft_seconds'],4),seconds=round(x['seconds'],4),prompt=x['usage']['prompt_tokens'],completion=x['usage']['completion_tokens'],cached=x['usage'].get('prompt_tokens_details',{}).get('cached_tokens',0)) for x in q.get('tests',[])],phases=out['numeric_phases']),indent=2))
