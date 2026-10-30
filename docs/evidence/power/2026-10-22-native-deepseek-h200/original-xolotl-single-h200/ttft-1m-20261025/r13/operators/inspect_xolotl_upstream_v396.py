"""Read the exact public upstream comparison; preserve source revisions without changing active builds."""
import json,time,urllib.request,urllib.error
from pathlib import Path
B=Path(__file__).resolve().parent
base='e9d0826d71c59072cbff77d2a31dd51f27ab7a12';head='0b9a0fc86dc55388c8c737bdc5fbccccf7273927'
url='https://api.github.com/repos/Indras-Mirror/Xolotl/compare/'+base+'...'+head
req=urllib.request.Request(url,headers={'User-Agent':'aster-deployment-source-review','Accept':'application/vnd.github+json'})
with urllib.request.urlopen(req,timeout=30) as r:raw=r.read(8*1024**2+1)
if len(raw)>8*1024**2:raise RuntimeError('bounded upstream comparison exceeded')
data=json.loads(raw)
if data['base_commit']['sha']!=base or data['commits'][-1]['sha']!=head:raise RuntimeError('exact upstream comparison revisions differ')
out=dict(unix=time.time(),phase='observed',url=url,base=base,head=head,status=data['status'],ahead_by=data['ahead_by'],total_commits=data['total_commits'],commits=[dict(sha=c['sha'],date=c['commit']['committer']['date'],message=c['commit']['message']) for c in data['commits']],
 files=[{k:f[k] for k in ['filename','status','additions','deletions','changes','patch'] if k in f} for f in data['files']],active_runtime_changed=False)
with (B/'xolotl-upstream-comparison-v396.json').open('x') as f:f.write(json.dumps(out,indent=2)+'\n')
print(json.dumps(dict(status=out['status'],ahead_by=out['ahead_by'],total_commits=out['total_commits'],commits=out['commits'],changed_files=[f['filename'] for f in out['files']]),ensure_ascii=False,indent=2))
