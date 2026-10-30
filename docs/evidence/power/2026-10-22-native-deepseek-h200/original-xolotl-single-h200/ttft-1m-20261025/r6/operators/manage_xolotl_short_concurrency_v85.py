"""Launch once or observe one bounded short-input concurrency diagnostic on the optimization model."""
import argparse
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, observe

parser=argparse.ArgumentParser();parser.add_argument('action',choices=['launch','observe']);action=parser.parse_args().action
intent=BASE/'xolotl-short-concurrency-v85-intent.json'
if action=='launch':
    if intent.exists():raise RuntimeError('exact diagnostic handle exists; observe it')
    prior=json.loads((BASE/'xolotl-engram-local-repeat-v84-controller.json').read_text())
    if prior['phase']!='passed' or prior['client_tests']!=7:raise RuntimeError('all exact repeat controls must finish first')
    service=json.loads((BASE/'xolotl-engram-local-repeat-v79-local_repeat-intent.json').read_text())
    client=json.loads((BASE/'xolotl-engram-local-repeat-v80-local_repeat-intent.json').read_text())
    reference=json.loads((BASE/'xolotl-draft-extended-v53-off-intent.json').read_text())
    resource=observe();reservation=json.loads((BASE/'xolotl-active-serving-reservation.json').read_text())
    if resource['phase']!='Running' or resource['job_uid']!=service['allocation_uid'] or resource['job_uid']==reservation['allocation_uid']:raise RuntimeError('exact independent diagnostic allocation differs')
    source=(BASE/'xolotl_draft_long_output_worker_v50.frozen.py').read_bytes()
    plan=dict(service_root=service['root'],base_url='http://'+service['pod_ip']+':30003/v1',reference_root=reference['root'],request_source_sha256=hashlib.sha256(source).hexdigest())
    files={'probe-worker.py':(BASE/'xolotl_short_concurrency_probe_v81.frozen.py').read_bytes(),'qualified-request-source.py':source,'probe-plan.json':(json.dumps(plan,indent=2)+'\n').encode()}
    record=dict(unix=time.time(),phase='prepared',root=service['root']+'/short-concurrency-probe-v81',allocation_uid=service['allocation_uid'],gpu_uuid=service['gpu_uuid'],
                formal_service_unchanged=True,full1m_input_tested=False,files=[dict(file=n,bytes=len(v),sha256=hashlib.sha256(v).hexdigest()) for n,v in files.items()])
    code='''import base64,hashlib,json,os,subprocess,time,urllib.request
from pathlib import Path
r=@R@;client=@C@;reference=@REF@;data=@DATA@;root=Path(r['root']);service=root.parent
if os.getuid()!=10250 or service.resolve()!=service or service.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive diagnostic root/owner differs')
q=json.loads((service/'service-longctx-v1-result.json').read_text())
if q['phase']!='serving' or q['gpu_uuid']!=r['gpu_uuid'] or q['context_tokens']!=1048576 or time.time()+360>q['lease_ends_unix']:raise RuntimeError('exact diagnostic model/finite lease differs')
for old in [client,reference]:
 p=Path('/proc/'+str(old['process']['pid'])+'/stat')
 alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==old['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
 if alive or json.loads((Path(old['root'])/'request-result.json').read_text())['phase']!='passed':raise RuntimeError('exact prior single clients must finish and pass')
key=(service/'credentials/api-key.txt').read_text().strip();request=urllib.request.Request(@SLOTS@,headers={'Authorization':'Bearer '+key})
with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request,timeout=5) as response:slots=json.load(response)
if not isinstance(slots,list) or not slots or any(row['is_processing'] for row in slots):raise RuntimeError('exact diagnostic slots must be idle')
os.umask(0o077);root.mkdir(mode=0o700)
for row in r['files']:
 raw=base64.b64decode(data[row['file']])
 if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen diagnostic source differs')
 with (root/row['file']).open('xb') as output:output.write(raw)
with (root/'probe-worker.log').open('xb') as log:
 p=subprocess.Popen(['timeout','--signal=TERM','--kill-after=15','300','python3','-u',str(root/'probe-worker.py')],cwd=root,stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
process=dict(pid=p.pid,start_ticks=int(Path('/proc/'+str(p.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),budget_seconds=300,namespace='jump host')
(root/'probe-process.json').write_text(json.dumps(process,indent=2)+chr(10));print(json.dumps(process))
'''.replace('@R@',repr(record)).replace('@C@',repr(client)).replace('@REF@',repr(reference)).replace('@SLOTS@',repr(plan['base_url'].removesuffix('/v1')+'/slots')).replace('@DATA@',repr({n:base64.b64encode(v).decode() for n,v in files.items()}))
    compile(code,'exact-short-concurrency-launch-v85','exec')
    with intent.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
    record['phase']='launch_requested';intent.write_text(json.dumps(record,indent=2)+'\n')
    try:record.update(process=jump(code),phase='launched')
    except BaseException:
        record['phase']='launch_outcome_requires_inspection';intent.write_text(json.dumps(record,indent=2)+'\n');raise
    intent.write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(dict(phase=record['phase'],process=record['process']),indent=2))
else:
    record=json.loads(intent.read_text());code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);out={'unix':time.time()};p=root/'probe-result.json'
if root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact diagnostic result owner differs')
if p.exists():
 if p.is_symlink() or p.stat().st_size>1048576:raise RuntimeError('bounded diagnostic result exceeded')
 out['result']=json.loads(p.read_text())
p=Path('/proc/'+str(r['process']['pid'])+'/stat')
out['exact_client_alive']=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
print(json.dumps(out))
'''.replace('@R@',repr(record))
    value=jump(code);(BASE/'xolotl-short-concurrency-v85-latest.json').write_text(json.dumps(value,indent=2)+'\n');q=value.get('result',{})
    print(json.dumps(dict(phase=q.get('phase'),tests=len(q.get('tests',[])),pairs=q.get('pairs'),serial_post_matched=q.get('serial_post_matched'),error=q.get('error'),exact_client_alive=value['exact_client_alive']),indent=2))
