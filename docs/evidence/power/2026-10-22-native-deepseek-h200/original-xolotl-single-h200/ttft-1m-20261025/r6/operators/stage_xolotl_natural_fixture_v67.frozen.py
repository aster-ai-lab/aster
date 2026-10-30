"""Freeze committed public repository text and prepare exact model-tokenizer fixtures on CPU."""
import base64
import hashlib
import json
import subprocess
import time
from pathlib import Path
from xolotl_trial_common import BASE, jump, lease

intent = BASE / 'xolotl-natural-fixtures-v67-intent.json'
if intent.exists():
    raise RuntimeError('natural fixture CPU task has an exact handle; observe it')
workspace = Path('D:/code/aster/.wt-h200-native-integration')
revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=workspace, text=True).strip()
names = ['docs/development-standards.md', '.specify/memory/constitution.md',
         'docs/spec-driven-development.md', 'README.md', 'docs/migration.md']
parts = []
sources = []
for name in names:
    raw = subprocess.check_output(['git', 'show', revision + ':' + name], cwd=workspace)
    sources.append(dict(file=name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()))
    parts.append(('\nDocument: ' + name + '\n').encode() + raw)
corpus = b'\n'.join(parts)
if len(corpus) > 512 * 1024:
    raise RuntimeError('public technical corpus byte bound exceeded')
adapter = json.loads((BASE / 'xolotl-api-hybrid-v52-intent.json').read_text())
root = lease()['remote'] + '/natural-fixtures-v67-cpu'
plan = dict(source_public_root=adapter['root'],
            api_sha256='f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba',
            corpus_sha256=hashlib.sha256(corpus).hexdigest(), source_revision=revision, source_files=sources)
files = {'prepare-plan.json': (json.dumps(plan, indent=2) + '\n').encode(), 'corpus.txt': corpus,
         'prepare-worker.py': (BASE / 'xolotl_natural_fixture_prepare_v67.frozen.py').read_bytes()}
record = dict(unix=time.time(), phase='prepared', root=root, gpu_executed=False, credentials_copied=False,
              source_revision=revision, source_files=sources,
              files=[dict(file=n, bytes=len(v), sha256=hashlib.sha256(v).hexdigest()) for n, v in files.items()])
code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive public fixture owner differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact public fixture CPU input differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
with (root/'prepare-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','120','python3','-u',str(root/'prepare-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=120,namespace='jump host')
(root/'prepare-process.json').write_text(json.dumps(process,indent=2)+'\\n');print(json.dumps(process))
'''.replace('@R@', repr(record)).replace('@DATA@', repr({n: base64.b64encode(v).decode() for n, v in files.items()}))
compile(code, 'exact-public-natural-fixture-v67', 'exec')
with intent.open('x') as output:
    output.write(json.dumps(record, indent=2) + '\n')
record['phase'] = 'launch_requested'
intent.write_text(json.dumps(record, indent=2) + '\n')
try:
    record['process'] = jump(code)
    record['phase'] = 'launched'
except BaseException:
    record['phase'] = 'launch_outcome_requires_inspection'
    intent.write_text(json.dumps(record, indent=2) + '\n')
    raise
intent.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(dict(phase=record['phase'], process=record['process'], gpu_executed=False)))
