"""Retain the unchanged public body with metadata from the actual protected API encoder."""
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease

ip = BASE / 'xolotl-canonical-code-fixture-v147-intent.json'
if ip.exists():
    raise RuntimeError('canonical public fixture has an exact handle; inspect it')
proof = json.loads((BASE / 'xolotl-literal-scan-cpu-v145r1-latest.json').read_text())
q = proof['cpu-result.json']
if proof['exact_cpu_process_alive'] or q['phase'] != 'passed' or not q['all_A_B_A_ids_exact']:
    raise RuntimeError('exact actual-API canonical encoder proof required')
source = json.loads((BASE / 'xolotl-code-fullwindow-fixture-v114-intent.json').read_text())
metadata = proof['fixture-metadata-canonical.json']
record = dict(unix=time.time(), phase='prepared', root=lease()['remote'] + '/canonical-code-fullwindow-v147',
    original_fixture_root=source['root'], metadata=metadata, body_changed=False, credentials_copied=False, gpu_executed=False)
code = '''import hashlib,json,os
from pathlib import Path
r=@R@;root=Path(r['root']);src=Path(r['original_fixture_root']);m=r['metadata']
if os.getuid()!=10250 or root.exists() or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or src.resolve()!=src or src.stat().st_uid!=10250:raise RuntimeError('exact exclusive canonical fixture owner differs')
os.umask(0o077);root.mkdir(mode=0o700)
for name in ['public-fixture.txt','public-fixture.txt.gz','corpus-source-manifest.json']:
 p=src/name
 if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>8*1024**2:raise RuntimeError('canonical public fixture source bound differs')
 raw=p.read_bytes()
 if name=='public-fixture.txt' and (len(raw)!=m['prompt_bytes'] or hashlib.sha256(raw).hexdigest()!=m['prompt_sha256']):raise RuntimeError('unchanged exact canonical public body differs')
 with (root/name).open('xb') as output:output.write(raw)
with (root/'fixture-metadata.json').open('x') as output:output.write(json.dumps(m,indent=2)+'\\n')
print(json.dumps(dict(phase='staged',body_changed=False,prompt_tokens=m['actual_prompt_tokens'],api_id_sha256=m['prompt_id_sha256'])))
'''.replace('@R@', repr(record))
with ip.open('x') as output:
    output.write(json.dumps(record, indent=2) + '\n')
try:
    record.update(result=jump(code), phase='staged')
except BaseException:
    record['phase'] = 'staging_outcome_requires_inspection'
    ip.write_text(json.dumps(record, indent=2) + '\n')
    raise
ip.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record['result']))
