"""Check append-only evidence, real fixture provenance, actual secrets and staged Git blob bytes."""
import ast
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import time

B=Path('D:/code/aster/target/h200-native');W=Path('D:/code/aster/.wt-h200-native-integration')
E=W/'docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200';D=E/'ttft-1m-20261025/r8'
manifest=json.loads((E/'manifest.json').read_bytes())
prior=json.loads(subprocess.check_output(['git','show','HEAD:'+str((E/'manifest.json').relative_to(W)).replace('\\','/')],cwd=W))
if manifest['files'][:len(prior['files'])]!=prior['files'] or len(prior['files'])!=2295:raise RuntimeError('historical2295 manifest rows changed')
if len({r['file'] for r in manifest['files']})!=len(manifest['files']):raise RuntimeError('duplicate evidence names')
keys=[]
for p in B.glob('access-private-*/api-key.txt'):
    value=p.read_bytes().strip()
    if len(value)>=24 and value not in keys:keys.append(value)
if not keys:raise RuntimeError('actual private credential negative controls absent')
for row in manifest['files']:
    raw=(E/row['file']).read_bytes()
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('retained evidence bytes differ: '+row['file'])
new=[]
for row in manifest['files'][len(prior['files']):]:
    path=E/row['file'];raw=path.read_bytes()
    if path.suffix in {'.o','.so','.dll','.exe','.nsys-rep','.ncu-rep','.sqlite','.gguf','.safetensors'}:raise RuntimeError('binary/private profiler/model payload refused')
    if len(raw)>2*1024**2 or any(key in raw for key in keys):raise RuntimeError('bounded actual credential gate refused')
    if path.suffix=='.json':json.loads(raw)
    if path.suffix=='.py':ast.parse(raw,filename=str(path))
    new.append((path.relative_to(W).as_posix(),row))
metadata=json.loads((D/'canonical-fixture/fixture-metadata.json').read_bytes())
body=gzip.decompress((E/'ttft-1m-20261025/r7/code-fixture/public-fixture.txt.gz').read_bytes())
if len(body)!=metadata['prompt_bytes'] or hashlib.sha256(body).hexdigest()!=metadata['prompt_sha256'] or metadata['actual_prompt_tokens']!=1047544 or metadata['corpus_repeated'] or metadata['body_changed'] or metadata['native_runtime_id_digest_captured']:
    raise RuntimeError('canonical API metadata must preserve exact public body/count and actual capture limits')
if metadata['prompt_id_sha256']!='b442a593a4066e3e99b663b670db4d7d624c471f1c5f6ee906e315c99d1969c4' or metadata['previous_direct_template_prompt_id_sha256']!='ebe492e5734df0265668f474ab674a282e03f61d8d4ded8d71343f2dc9b9e6ca':raise RuntimeError('canonical/historical ID digest provenance differs')
summary=json.loads((D/'round-summary.json').read_bytes())
if summary['fullwindow_suite_requests']!=20 or summary['actual1m_requests']!=18 or summary['c2_full_output_identity_gate_passed'] or summary['simd_model_requests']!=120 or summary['goal_complete'] or summary['experimental_candidates_promoted']:raise RuntimeError('real request counts or failed/unfinished acceptance was misrepresented')
requests=b''.join((':'+name+'\n').encode() for name,row in new)
result=subprocess.run(['git','cat-file','--batch'],cwd=W,input=requests,capture_output=True,timeout=120,check=True)
if len(result.stdout)>32*1024**2:raise RuntimeError('bounded staged blob output exceeded')
cursor=0
for name,row in new:
    end=result.stdout.index(b'\n',cursor);header=result.stdout[cursor:end].split();cursor=end+1
    if len(header)!=3 or header[1]!=b'blob':raise RuntimeError('exact staged blob missing: '+name)
    size=int(header[2]);raw=result.stdout[cursor:cursor+size];cursor+=size+1
    if size!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('Git filters changed frozen evidence bytes: '+name)
if cursor!=len(result.stdout):raise RuntimeError('unexpected staged blob bytes')
roles=json.loads((W/'docs/source-manifest.json').read_bytes())['original_xolotl_h200_20261024']['current_serving_optimization_roles']
if roles['serving']['allocation_uid']!='1cfa236c-40ba-408f-a45e-003e0261bdf5' or roles['optimization']['allocation_uid']!='7455d989-8255-4f7c-ae8f-3cc0aa5928e4' or roles['goal_met'] or roles['maximum_allocated_h200']!=2:raise RuntimeError('exact current C11/C10 roles differ')
receipt=dict(unix=time.time(),phase='verified',historical_entries_preserved=len(prior['files']),new_evidence_blobs_verified=len(new),all_manifest_entries_verified=len(manifest['files']),
    actual_credential_negative_controls=len(keys),credential_occurrences=0,canonical_public_input_tokens=1047544,actual1m_native_ID_digest_captured=False,
    request_count_verified=dict(fullwindow_suite=20,actual1m=18,simd_standard=120),failed_C2_gate_preserved=True,
    weights_binaries_raw_profilers_published=False,experimental_candidates_promoted=False,goal_complete=False)
with (B/'h200-r8-publication-v160.json').open('x') as output:output.write(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt))
