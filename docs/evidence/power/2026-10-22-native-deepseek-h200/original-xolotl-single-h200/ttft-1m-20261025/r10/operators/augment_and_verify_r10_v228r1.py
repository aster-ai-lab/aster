"""Include parent parser dependencies, run published hermetic tests and verify exact staged bytes."""
import ast
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

B=Path(__file__).resolve().parent;W=Path('D:/code/aster/.wt-h200-native-integration')
E=W/'docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200';D=E/'ttft-1m-20261025/r10'
def git(*args):return subprocess.check_output(['git',*args],cwd=W)
parent=json.loads(git('show','HEAD:'+str((E/'manifest.json').relative_to(W)).replace('\\','/')))
if len(parent['files'])!=2645 or git('diff','--cached','--name-only').strip():raise RuntimeError('exact clean-index R9 parent required')
keys=list({p.read_bytes().strip() for p in B.glob('access-private-*/api-key.txt') if len(p.read_bytes().strip())>=24})
if len(keys)<2:raise RuntimeError('both actual key negative controls required')
m=json.loads((D/'manifest.json').read_text())
def put(raw,name,source):
    if len(raw)>2*1024**2 or any(key in raw for key in keys):raise RuntimeError('bounded actual-key publication gate refused: '+name)
    if name.endswith('.py'):ast.parse(raw)
    path=D/name
    path.parent.mkdir(parents=True,exist_ok=True)
    if path.exists():
        if path.read_bytes()!=raw:raise RuntimeError('existing frozen publication bytes differ')
    else:
        with path.open('xb') as out:out.write(raw)
    m['files'].append(dict(file=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),source=source))
for name in ['aster_ncu_metrics_v171.py','test_ncu_metrics_v171.py','aster_ncu_query_v178.py','test_ncu_query_v178.py']:
    raw=(B/name).read_bytes();prior=E/'ttft-1m-20261025/r9/operators'/name
    if prior.read_bytes()!=raw:raise RuntimeError('exact R9 parser dependency bytes differ')
    put(raw,'operators/'+name,'exact R9 dependency '+name)
# The revised protocol test imports only its already retained V197 helper.
put((B/'augment_and_verify_r10_v228.py').read_bytes(),'operators/augment_and_verify_r10_v228.py','augment_and_verify_r10_v228.py')
put((B/'augment_and_verify_r10_v228r1.py').read_bytes(),'operators/augment_and_verify_r10_v228r1.py','corrected parent-directory/resumption operator')
command=[sys.executable,'-B','-X','utf8','-m','unittest','-v','test_ncu_metrics_v171','test_ncu_query_v178','test_native_resume_v197','test_ncu_wide_metrics_v206']
test=subprocess.run(command,cwd=D/'operators',capture_output=True,text=True,timeout=30)
if test.returncode or 'Ran 25 tests' not in test.stderr or not test.stderr.rstrip().endswith('OK'):raise RuntimeError('exact25 published hermetic parser methods failed: '+test.stderr[-4000:])
put((json.dumps(dict(command=command,returncode=test.returncode,tests=25,stderr=test.stderr,model_or_GPU_executed=False),indent=2)+'\n').encode(),'public-diagnostics/parser-tests25.json','published R10 operators test run')
(D/'manifest.json').write_text(json.dumps(m,indent=2)+'\n',encoding='utf-8',newline='\n')
manifest=json.loads((E/'manifest.json').read_text());manifest['files']=list(parent['files'])
for path in sorted(D.rglob('*')):
    if path.is_file():
        raw=path.read_bytes();manifest['files'].append(dict(file=path.relative_to(E).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
oldraw=(E/'manifest.json').read_bytes();newline='\r\n' if b'\r\n' in oldraw else '\n'
(E/'manifest.json').write_bytes((json.dumps(manifest,indent=2)+'\n').replace('\n',newline).encode())
for row in manifest['files']:
    raw=(E/row['file']).read_bytes()
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('immutable complete manifest hash differs: '+row['file'])
for row in m['files']:
    raw=(D/row['file']).read_bytes()
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('R10 local manifest hash differs')
expected={p.relative_to(W).as_posix() for p in D.rglob('*') if p.is_file()}
expected.update({(E/'manifest.json').relative_to(W).as_posix(),'docs/source-manifest.json'})
subprocess.run(['git','add','--',D.relative_to(W).as_posix(),(E/'manifest.json').relative_to(W).as_posix(),'docs/source-manifest.json'],cwd=W,check=True)
staged={name.decode() for name in git('diff','--cached','--name-only','-z').split(b'\0') if name}
if staged!=expected:raise RuntimeError('exact R10 staged path allowlist differs')
for name in sorted(staged):
    disk=(W/name).read_bytes();blob=git('show',':'+name)
    if disk!=blob or any(key in blob for key in keys):raise RuntimeError('staged exact-byte/actual-key gate failed: '+name)
check=subprocess.run(['git','diff','--cached','--check'],cwd=W,capture_output=True,text=True)
if check.returncode:raise RuntimeError('staged whitespace check failed: '+check.stdout[-4000:]+check.stderr[-1000:])
summary=json.loads((D/'round-summary.json').read_text())
if summary['goal_complete'] or summary['single_request300_met'] or summary['current_model_comparison']['model_performance_gain_qualified'] or summary['model_hardware_counters_collected'] or not summary['formal_C13_restored']:raise RuntimeError('R10 measured-vs-pending claims differ')
receipt=dict(unix=time.time(),phase='passed',parent_HEAD=git('rev-parse','HEAD').decode().strip(),
    previous_manifest_rows_preserved=2645,verified_manifest_rows=len(manifest['files']),new_R10_files=sum(p.is_file() for p in D.rglob('*')),
    staged_paths=len(staged),staged_Git_blob_bytes_equal=True,actual_key_negative_controls=len(keys),actual_key_occurrences=0,
    published_hermetic_methods=25,published_test_returncode=0,operator_GPU_cases_passed=38,model_gain_qualified=False,
    model_hardware_counters_collected=False,formal_C13_recovery_verified=True,Rust_workspace_checks_claimed=False,goal_complete=False)
with (B/'h200-r10-publication-verification-v228r1.json').open('x') as out:out.write(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt,indent=2))
