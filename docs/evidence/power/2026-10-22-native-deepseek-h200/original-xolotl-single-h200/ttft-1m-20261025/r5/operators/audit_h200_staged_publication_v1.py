"""Check index blobs without printing credential contents or private client state."""
import hashlib,json,subprocess,time
from pathlib import Path
BASE=Path('D:/code/aster/target/h200-native');WORK=Path('D:/code/aster/.wt-h200-native-integration')
def git(*args):return subprocess.check_output(['git',*args],cwd=WORK)
paths=[p.decode() for p in git('diff','--cached','--name-only','-z').split(b'\0') if p]
allowed=['.gitattributes','.github/actions/checkout-studio/action.yml','Cargo.toml','Cargo.lock','apps/gateway/src/entrypoint/protocol/','crates/deepseek/','crates/power','crates/third-party/serde-saphyr/','crates/third-party/saphyr-parser-bw/','scripts/h200_native/','scripts/h200_official/','scripts/h200_xq/','specs/009-power-native-deepseek-h200/','docs/evidence/power/','docs/migration.md','docs/source-manifest.json']
for p in paths:
    if not any(p==a or (a.endswith('/') and p.startswith(a)) for a in allowed):raise RuntimeError('unreviewed staged path: '+p)
    if p.endswith(('.gguf','.safetensors','.pt','.pem','.pfx','.exe','.dll','.so')) or p.startswith('apps/studio/'):raise RuntimeError('forbidden runtime artifact: '+p)
credentials=[]
for p in BASE.glob('access-private-*/api-key.txt'):
    raw=p.read_bytes().strip()
    if len(raw)>=24 and raw not in credentials:credentials.append(raw)
if not credentials:raise RuntimeError('actual credential negative control missing')
entries=[]
path_set=set(paths)
for item in git('ls-files','--stage','-z').split(b'\0'):
    if not item:continue
    head,p=item.split(b'\t',1);mode,sha,stage=head.split()
    if p.decode() not in path_set:continue
    if stage!=b'0':raise RuntimeError('unmerged staged entry')
    if mode==b'160000':
        if p!=b'crates/power' or sha!=b'c891f930bcbc06b52175ad4fe0247cc2c1f9b412':raise RuntimeError('unexpected gitlink')
        continue
    entries.append((p.decode(),sha.decode()))
unique=list(dict.fromkeys(sha for _,sha in entries))
q=subprocess.run(['git','cat-file','--batch'],cwd=WORK,input=('\n'.join(unique)+'\n').encode(),capture_output=True,check=True)
offset=0;blobs={};bad=[]
for sha in unique:
    e=q.stdout.index(b'\n',offset);h=q.stdout[offset:e].split();n=int(h[2]);raw=q.stdout[e+1:e+1+n];offset=e+2+n
    if h[0].decode()!=sha or h[1]!=b'blob':raise RuntimeError('unexpected index object')
    if len(raw)>8*1024**2:raise RuntimeError('staged source/artifact bound exceeded')
    if any(key in raw for key in credentials):bad.append(sha)
    blobs[sha]=raw
if bad:raise RuntimeError('credential found in staged blob; contents suppressed')
raw_count=0
for p,sha in entries:
    if p.startswith(('docs/evidence/power/2026-10-22-native-deepseek-h200/','docs/evidence/power/2026-10-22-h200-official/','docs/evidence/power/2026-10-24-h200-submit/','crates/deepseek/fixtures/')):
        if hashlib.sha256(blobs[sha]).digest()!=hashlib.sha256((WORK/p).read_bytes()).digest():raise RuntimeError('index byte identity differs: '+p)
        raw_count+=1
manifest_root=WORK/'docs/evidence/power/2026-10-22-native-deepseek-h200/original-xolotl-single-h200'
for row in json.loads((manifest_root/'manifest.json').read_text())['files']:
    raw=(manifest_root/row['file']).read_bytes()
    if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen evidence manifest differs: '+row['file'])
out=dict(unix=time.time(),phase='passed',staged_paths=len(paths),unique_blobs=len(unique),total_index_blob_bytes=sum(len(x) for x in blobs.values()),credential_negative_controls=len(credentials),credential_occurrences=0,exact_byte_files=raw_count,forbidden_runtime_artifacts=0,power_revision='c891f930bcbc06b52175ad4fe0247cc2c1f9b412')
(BASE/'h200-final-staged-audit-v1.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
