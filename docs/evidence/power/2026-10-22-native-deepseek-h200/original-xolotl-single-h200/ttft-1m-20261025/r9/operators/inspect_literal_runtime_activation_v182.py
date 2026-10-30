"""Read only exact declared public startup flags, source hashes and disabled-helper messages."""
import json
from xolotl_trial_common import BASE,owned
ip=BASE/'xolotl-literal-runtime-v182.json'
if ip.exists():raise RuntimeError('exact live activation inspection already retained')
r=json.loads((BASE/'xolotl-literal-service-v149-literal-intent.json').read_text())
code='''import hashlib,json,time
from pathlib import Path
r=@R@;root=Path(r['root']);q=json.loads((root/'service-longctx-v1-result.json').read_text());record=q['api_process']
stat=Path('/proc/'+str(record['pid'])+'/stat')
alive=stat.exists() and stat.stat().st_uid==10250 and int(stat.read_text().split(') ',1)[1].split()[19])==record['start_ticks'] and stat.read_text().split(') ',1)[1].split()[0]!='Z'
result=dict(unix=time.time(),phase=q['phase'],exact_api_alive=alive,allocation_uid=r['allocation_uid'],GPU_executed=False,model_request_sent=False)
if alive:
 p=stat.parent/'environ';raw=p.read_bytes()
 if len(raw)>1048576:raise RuntimeError('bounded exact API environment exceeded')
 entries=[value.split(b'=',1) for value in raw.split(b'\\0') if b'=' in value]
 result['public_startup_flags']={name.decode():value.decode() for name,value in entries if name in [b'ASTER_XQ_LITERAL_SCAN',b'ASTER_XQ_EXACT_TOKEN_CACHE']}
paths=['serve/frontend.py','serve/server.py','tools/v41/serve/serve_ds41.py','tools/v41/serve/aster_literal_scan.py']
result['source_hashes']=[]
for name in paths:
 p=root/'source'/name
 if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>1048576:raise RuntimeError('bounded exact live source differs')
 result['source_hashes'].append(dict(file=name,bytes=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest()))
messages=['literal scan disabled: pinned frontend source SHA differs','literal scan disabled: pinned server source SHA differs','literal scan disabled: pinned server unmark alias differs']
result['disabled_warning_counts']={message:0 for message in messages}
for name in ['api-longctx-v1-attempt1.stdout.log','api-longctx-v1-attempt1.stderr.log']:
 p=root/name
 if p.exists():
  if p.is_symlink() or p.stat().st_size>8*1024**2:raise RuntimeError('bounded exact live startup log exceeded')
  text=p.read_text(errors='replace')
  for message in messages:result['disabled_warning_counts'][message]+=text.count(message)
print(json.dumps(result))
'''.replace('@R@',repr(r))
result=owned(code)
with ip.open('x') as output:output.write(json.dumps(result,indent=2)+'\n')
print(json.dumps(result,indent=2))
