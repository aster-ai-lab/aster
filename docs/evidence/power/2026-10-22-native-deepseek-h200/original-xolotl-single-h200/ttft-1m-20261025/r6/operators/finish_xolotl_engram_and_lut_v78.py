"""Finish the exact resumed GPFS-B handle, then qualify one decoder candidate."""
import hashlib
import json
import subprocess
import sys
import time
from xolotl_trial_common import BASE, observe, owned

PATH=BASE/'xolotl-engram-lut-v78-controller.json'
prior=json.loads((BASE/'xolotl-engram-comparison-v71-controller.json').read_text())
if prior['phase']!='requires_inspection' or len(prior['profiles'])!=2:
    raise RuntimeError('exact two completed directory profiles and retained bootstrap refusal required')
STATE=dict(unix=time.time(),phase='resuming_exact_gpfs_b',profiles=prior['profiles'],
           original_controller_retained=True,formal_service_unchanged=True,actual1m_input_tested=False,
           speed_300_qualified=False,source_sha256=hashlib.sha256(__import__('pathlib').Path(__file__).read_bytes()).hexdigest())
with PATH.open('x') as output:output.write(json.dumps(STATE,indent=2)+'\n')


def save():
    STATE['observed_unix']=time.time()
    tmp=PATH.with_suffix('.tmp');tmp.write_text(json.dumps(STATE,indent=2)+'\n');tmp.replace(PATH)


def run(script,*args):
    result=subprocess.run([sys.executable,'-X','utf8',str(BASE/script),*args],cwd=BASE,capture_output=True,text=True,timeout=100)
    if result.returncode:
        raise RuntimeError(script+' '+str(args)+' exit'+str(result.returncode)+': '+result.stderr[-3500:])


def service(profile,lut,target,seconds):
    script='manage_xolotl_engram_lut_service_v75.frozen.py' if lut else 'manage_xolotl_engram_disk_v68.frozen.py'
    prefix='xolotl-engram-lut-v75-' if lut else 'xolotl-engram-disk-v68-'
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        run(script,profile,'observe')
        value=json.loads((BASE/(prefix+profile+'-latest.json')).read_text());q=value.get('service-longctx-v1-result.json',{})
        STATE.update(service_phase=q.get('phase'),profile=profile);save()
        if target=='serving' and q.get('phase')=='serving' and value['exact_supervisor_alive']:return q
        if target=='stopped' and q.get('phase')=='stopped' and q.get('api_reaped') and not value['exact_supervisor_alive']:return q
        if q.get('phase')=='failed' or target=='serving' and not value['exact_supervisor_alive']:raise RuntimeError('exact model service failed/disappeared; inspect it')
        time.sleep(15)
    raise RuntimeError('model observation budget expired; inspect same handle')


def client(profile,lut,extended):
    if lut:
        script='manage_xolotl_engram_lut_extended_v77.frozen.py' if extended else 'manage_xolotl_engram_lut_natural_v76.frozen.py'
        prefix='xolotl-engram-lut-extended-v77-' if extended else 'xolotl-engram-lut-natural-v76-'
    else:
        script='manage_xolotl_engram_extended_v70.frozen.py' if extended else 'manage_xolotl_engram_natural_v69.frozen.py'
        prefix='xolotl-engram-extended-v70-' if extended else 'xolotl-engram-natural-v69-'
    run(script,profile,'launch')
    end=time.monotonic()+620
    while time.monotonic()<end:
        run(script,profile,'observe')
        value=json.loads((BASE/(prefix+profile+'-latest.json')).read_text());q=value.get('result',{})
        STATE.update(client_phase=q.get('phase'),client_tests=len(q.get('tests',[])));save()
        if q.get('phase')=='failed':raise RuntimeError('exact comparison failed: '+str(q.get('error')))
        if not value['exact_client_alive']:
            if q.get('phase')!='passed':raise RuntimeError('exact comparison stopped without passing')
            return q
        time.sleep(20)
    raise RuntimeError('client observation budget expired; inspect same handle')


def wait_link():
    r=json.loads((BASE/'xolotl-engram-fp8-link-v74-intent.json').read_text());end=time.monotonic()+190
    while time.monotonic()<end:
        code='''import json,time
from pathlib import Path
r=@R@;root=Path(r['root']);p=Path('/proc/'+str(r['process']['pid'])+'/stat')
alive=p.exists() and p.stat().st_uid==10250 and int(p.read_text().split(') ',1)[1].split()[19])==r['process']['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'
q=root/'native-link-result.json'
if q.is_symlink() or q.stat().st_size>65536:raise RuntimeError('bounded native link receipt differs')
print(json.dumps({'unix':time.time(),'exact_link_process_alive':alive,'native-link-result.json':json.loads(q.read_text())}))
'''.replace('@R@',repr(r))
        observe();v=owned(code);(BASE/'xolotl-engram-fp8-link-v74-latest.json').write_text(json.dumps(v,indent=2)+'\n')
        q=v['native-link-result.json'];STATE['link_phase']=q.get('phase');save()
        if q.get('phase')=='failed':raise RuntimeError('exact native link failed: '+str(q.get('error')))
        if not v['exact_link_process_alive']:
            if q.get('phase')!='linked':raise RuntimeError('exact native link stopped without result')
            return q
        time.sleep(10)
    raise RuntimeError('native link observation budget expired; inspect same handle')


try:
    service('gpfs_b',False,'serving',300)
    STATE['phase']='gpfs_b_natural';save();natural=client('gpfs_b',False,False)
    STATE['phase']='gpfs_b_extended';save();extended=client('gpfs_b',False,True)
    STATE['profiles'].append(dict(profile='gpfs_b',cases=natural['cases'],natural_tests=len(natural['tests']),extended_tests=len(extended['tests']),
        native_tool_roundtrip_passed=extended['native_tool_roundtrip_passed'],exact_long_output_reference_matched=extended['exact_long_output_reference_matched'],long_summary=extended['long_summary']))
    summary=dict(unix=time.time(),phase='passed',profiles=STATE['profiles'],formal_service_unchanged=True,actual1m_input_tested=False,
        original_controller_phase='requires_inspection',original_refusal_retained=True,exact_unstarted_gpfs_b_recovered=True,resource_deadline_extended=False,
        scope='One finite same-GPU GPFS/local/GPFS comparison; actual8244/25671 public technical input and1024 output; source OS cache not evicted')
    with (BASE/'xolotl-engram-comparison-v71r1-controller.json').open('x') as output:output.write(json.dumps(summary,indent=2)+'\n')
    STATE.update(phase='draining_completed_gpfs_b',directory_comparison_passed=True);save()
    run('manage_xolotl_engram_disk_v68.frozen.py','gpfs_b','drain');service('gpfs_b',False,'stopped',120)
    STATE['phase']='linking_exact_decoder';save();run('launch_xolotl_engram_fp8_link_v74.py');link=wait_link()
    STATE.update(phase='launching_decoder',candidate_binary=link['binary']);save();run('launch_xolotl_engram_lut_v75.frozen.py','lut_local')
    service('lut_local',True,'serving',300)
    STATE['phase']='decoder_natural';save();natural=client('lut_local',True,False)
    STATE['phase']='decoder_extended';save();extended=client('lut_local',True,True)
    STATE.update(phase='passed',decoder_natural_cases=natural['cases'],decoder_natural_tests=len(natural['tests']),decoder_extended_tests=len(extended['tests']),
        decoder_native_tool_roundtrip_passed=extended['native_tool_roundtrip_passed'],decoder_exact_long_output_reference_matched=extended['exact_long_output_reference_matched'],
        decoder_long_summary=extended['long_summary'],finished_unix=time.time());save()
except BaseException as error:
    STATE.update(phase='requires_inspection',error=type(error).__name__+': '+str(error),finished_unix=time.time());save();raise
