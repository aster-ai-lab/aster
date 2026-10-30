"""Execute finite exhaustive CPU equivalence tests without using the optimization pod CPUs."""
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease

intent = BASE / 'xolotl-engram-fp8-cpu-v72-intent.json'
if intent.exists():
    raise RuntimeError('exact CPU decoder handle exists; inspect it')
source = BASE / 'engram-fp8-lut-v72'
manifest = json.loads((source/'source-manifest.json').read_text())
files = {row['file']:(source/row['file']).read_bytes() for row in manifest['files']}
for row in manifest['files']:
    if len(files[row['file']])!=row['bytes'] or hashlib.sha256(files[row['file']]).hexdigest()!=row['sha256']:
        raise RuntimeError('exact FP8 candidate source differs')
record = dict(unix=time.time(),phase='prepared',root=lease()['remote']+'/cpu-engram-fp8-lut-v72',
              files=manifest['files'],gpu_code_executed=False,model_configuration_changed=False)
code = '''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);data=@DATA@
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive CPU FP8 root/owner differs')
os.umask(0o077);root.mkdir(mode=0o700)
p=Path('/proc/'+str(os.getpid())+'/stat');process={'pid':os.getpid(),'start_ticks':int(p.read_text().split(') ',1)[1].split()[19]),'uid':os.getuid(),'unix':time.time(),'namespace':'jump host'}
(root/'cpu-process.json').write_text(json.dumps(process,indent=2)+chr(10))
state={'phase':'preflight','unix':time.time(),'process':process,'runs':[],'gpu_code_executed':False,'performance_claimed':False}
def save():
 tmp=root/'cpu-result.tmp';tmp.write_text(json.dumps(state,indent=2)+chr(10));tmp.replace(root/'cpu-result.json')
save()
try:
 for row in r['files']:
  raw=base64.b64decode(data[row['file']])
  if len(raw)!=row['bytes'] or hashlib.sha256(raw).hexdigest()!=row['sha256']:raise RuntimeError('frozen FP8 CPU input differs')
  with (root/row['file']).open('xb') as output:output.write(raw)
 env=dict(os.environ,CUDA_VISIBLE_DEVICES='',OMP_NUM_THREADS='1',ASAN_OPTIONS='detect_leaks=1:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1')
 for name,extra in [('control0',['-O3','-DASTER_XQ_ENGRAM_FP8_LUT=0']),('enabled1',['-O3','-DASTER_XQ_ENGRAM_FP8_LUT=1']),('enabled1-sanitized',['-O1','-g','-DASTER_XQ_ENGRAM_FP8_LUT=1','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
  state.update(phase='compiling',current=name);save()
  command=['nice','-n','19','/usr/bin/c++','-std=c++17','-frounding-math','-Wall','-Wextra','-Werror',*extra,'fp8_exact_test.cpp','-o',name]
  compiled=subprocess.run(command,cwd=root,env=env,capture_output=True,text=True,timeout=18)
  (root/(name+'-compile.log')).write_text(compiled.stdout+compiled.stderr)
  if compiled.returncode:raise RuntimeError('FP8 CPU compilation failed '+name+': '+compiled.stderr[-1500:])
  state['phase']='testing';save()
  tested=subprocess.run(['nice','-n','19',str(root/name)],cwd=root,env=env,capture_output=True,text=True,timeout=8)
  (root/(name+'-test.log')).write_text(tested.stdout+tested.stderr)
  if tested.returncode:raise RuntimeError('FP8 exact/sanitizer comparison failed '+name+': '+tested.stderr[-1500:])
  result=json.loads(tested.stdout)
  if result['phase']!='passed' or result['scaled_cases']!=262144 or result['conversion_cases']!=1024 or not result['bitwise_equal']:raise RuntimeError('exhaustive FP8 counts incomplete')
  state['runs'].append({'name':name,'command':command,'result':result,'stderr_empty':not tested.stderr});save()
 command=['nice','-n','19','/usr/bin/c++','-std=c++17','-DASTER_XQ_ENGRAM_FP8_LUT=2','fp8_exact_test.cpp','-o','invalid2']
 invalid=subprocess.run(command,cwd=root,env=env,capture_output=True,text=True,timeout=12)
 if invalid.returncode==0 or 'ASTER_XQ_ENGRAM_FP8_LUT must be 0 or 1' not in invalid.stderr:raise RuntimeError('unknown compile-time mode was not refused')
 state.update(phase='passed',invalid_mode_refused=True,exact_fp8_all_encodings_and_scales_passed=True,source_weight_bytes_changed=False,finished_unix=time.time());save()
except BaseException as error:
 state.update(phase='failed',error=type(error).__name__+': '+str(error),finished_unix=time.time());save();raise
print(json.dumps(state))
'''.replace('@R@',repr(record)).replace('@DATA@',repr({name:base64.b64encode(raw).decode() for name,raw in files.items()}))
compile(code,'engram-fp8-exact-CPU-v72','exec')
with intent.open('x') as output:
    output.write(json.dumps(record,indent=2)+'\n')
record['phase']='execution_requested';intent.write_text(json.dumps(record,indent=2)+'\n')
try:
    result=jump(code,55)
    record.update(phase=result['phase'],process=result['process'])
    (BASE/'xolotl-engram-fp8-cpu-v72-latest.json').write_text(json.dumps(result,indent=2)+'\n')
except BaseException:
    record['phase']='execution_outcome_requires_inspection';intent.write_text(json.dumps(record,indent=2)+'\n');raise
intent.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(result,indent=2))
