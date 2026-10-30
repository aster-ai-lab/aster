"""Build corpus from the verified immutable public source tar; stage one bounded CPU tokenizer job."""
import base64
import gzip
import hashlib
import json
import tarfile
import time
from pathlib import PurePosixPath
from xolotl_trial_common import BASE, jump, lease

intent=BASE/'xolotl-code-fullwindow-fixture-v114-intent.json'
if intent.exists():raise RuntimeError('code fixture has an exact handle; observe it')
source=json.loads((BASE/'xolotl-api-hybrid-v52-intent.json').read_text())
prior=json.loads((BASE/'xolotl-api-hybrid-v52-latest.json').read_text())
if prior['cpu-preflight-result.json']['phase']!='passed' or prior['exact_cpu_process_alive']:
    raise RuntimeError('completed exact CPU tokenizer source required')
sources=json.loads((BASE/'xolotl-trial-source-e9d0826d71c5/sources.json').read_text())['sources']
rows=[r for r in sources if r['repository']=='Indras-Mirror/Xolotl' and r['revision']=='e9d0826d71c59072cbff77d2a31dd51f27ab7a12']
if len(rows)!=1:raise RuntimeError('pinned public Xolotl archive identity differs')
archive=BASE/'xolotl-trial-source-e9d0826d71c5'/('Xolotl-'+rows[0]['revision']+'.tar.gz')
raw=archive.read_bytes()
if len(raw)!=rows[0]['bytes'] or hashlib.sha256(raw).hexdigest()!=rows[0]['sha256']:
    raise RuntimeError('whole pinned public source archive differs')
prefix='Xolotl-'+rows[0]['revision']+'/'
allowed_roots=['tools/v41/engine/','src/kernels/','include/strata/kernels/']
allowed_extensions={'.cpp','.hpp','.cu','.cuh'}
files=[];seen=set();license_text=None
with tarfile.open(archive,'r:gz') as tar:
    for member in tar.getmembers():
        if not member.name.startswith(prefix):continue
        path=member.name[len(prefix):]
        if path=='LICENSE':
            if not member.isfile() or member.size>16384:raise RuntimeError('public license entry differs')
            license_text=tar.extractfile(member).read().decode('utf-8',errors='strict');continue
        if not any(path.startswith(r) for r in allowed_roots) or PurePosixPath(path).suffix not in allowed_extensions:continue
        if not member.isfile() or '..' in PurePosixPath(path).parts or not 0<member.size<=1024**2:
            raise RuntimeError('bounded public code entry refused')
        blob=tar.extractfile(member).read();digest=hashlib.sha256(blob).hexdigest()
        if len(blob)!=member.size:raise RuntimeError('public archive entry length differs')
        if digest in seen:continue
        seen.add(digest);files.append(dict(file=path,bytes=len(blob),sha256=digest,content=blob.decode('utf-8',errors='strict')))
if license_text is None or not 1<=len(files)<=512 or sum(r['bytes'] for r in files)>10*1024**2:
    raise RuntimeError('pinned license/file count/corpus bound differs')
files.sort(key=lambda r:(next(i for i,p in enumerate(allowed_roots) if r['file'].startswith(p)),r['file']))
corpus=dict(repository='https://github.com/Indras-Mirror/Xolotl',revision=rows[0]['revision'],
            source_archive_sha256=rows[0]['sha256'],license=license_text,files=files)
payload=(json.dumps(corpus,ensure_ascii=False,separators=(',',':'))+'\n').encode()
packed=gzip.compress(payload,mtime=0)
if len(payload)>12*1024**2 or len(packed)>3*1024**2:raise RuntimeError('public corpus archive bound exceeded')
key=(BASE/'access-private-xolotl-9c996c5a434b/api-key.txt').read_bytes().strip()
if not key or key in payload:raise RuntimeError('actual credential negative control failed')
with (BASE/'public-code-corpus-v114.json.gz').open('xb') as output:output.write(packed)
root=lease()['remote']+'/code-fullwindow-fixture-v114-cpu'
plan=dict(source_public_root=source['root'],api_sha256='f062d9d39131959da9c981b6704e2ec27f89d8e2806730062dcabc3eb27682ba',
          source_revision=rows[0]['revision'],corpus_archive_sha256=hashlib.sha256(packed).hexdigest())
inputs={'prepare-plan.json':(json.dumps(plan,indent=2)+'\n').encode(),'public-code-corpus.json.gz':packed}
for name in ['xolotl_code_fullwindow_prepare_v114.frozen.py','xolotl_fullwindow_long_fixture_v54.py','test_xolotl_fullwindow_long_fixture_v54.py']:
    inputs[name]=(BASE/name).read_bytes()
record=dict(unix=time.time(),phase='prepared',root=root,source_public_root=source['root'],source_revision=rows[0]['revision'],
    gpu_executed=False,credentials_copied=False,actual_credential_negative_control_passed=True,source_files=len(files),
    source_bytes=sum(r['bytes'] for r in files),corpus_repeated=False,
    files=[dict(file=n,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()) for n,b in inputs.items()])
code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive CPU code fixture owner differs')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('exact CPU fixture input differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
with (root/'prepare-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','240','python3','-u',str(root/'xolotl_code_fullwindow_prepare_v114.frozen.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=240,namespace='jump host')
(root/'prepare-process.json').write_text(json.dumps(process,indent=2)+chr(10));print(json.dumps(process))
'''.replace('@R@',repr(record)).replace('@DATA@',repr({n:base64.b64encode(b).decode() for n,b in inputs.items()}))
compile(code,'cpu-code-fullwindow-stage-v114','exec')
with intent.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
record['phase']='launch_requested';intent.write_text(json.dumps(record,indent=2)+'\n')
try:record['process']=jump(code,80);record['phase']='launched'
except BaseException:
    record['phase']='launch_outcome_requires_inspection';intent.write_text(json.dumps(record,indent=2)+'\n');raise
intent.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(dict(phase=record['phase'],process=record['process'],source_files=record['source_files'],source_bytes=record['source_bytes'],corpus_archive_bytes=len(packed),credentials_copied=False,gpu_executed=False)))
