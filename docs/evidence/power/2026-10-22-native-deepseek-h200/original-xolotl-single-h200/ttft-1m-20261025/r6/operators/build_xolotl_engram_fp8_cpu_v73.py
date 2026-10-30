"""Reproduce the exact qualified Engram object before compiling the opt-in decoder on jump CPUs."""
import base64
import hashlib
import json
import time
from xolotl_trial_common import BASE, jump, lease

intent=BASE/'xolotl-engram-fp8-build-v73-intent.json'
if intent.exists():
    raise RuntimeError('exact Engram compilation handle exists; inspect it')
cpu=json.loads((BASE/'xolotl-engram-fp8-cpu-v72-latest.json').read_text())
if cpu['phase']!='passed' or not cpu['exact_fp8_all_encodings_and_scales_passed']:
    raise RuntimeError('exhaustive actual decoder equivalence must pass first')
parent=json.loads((BASE/'xolotl-draft-cumulative-v49-generator-parent.json').read_text())
old=json.loads((BASE/'xolotl-indexer-gpu-build-v40r2-intent.json').read_text())
native=json.loads((BASE/'xolotl-draft-cumulative-v49-native-link-intent.json').read_text())
source=(BASE/'engram-fp8-lut-v72/ds41_engram.cpp').read_bytes()
record=dict(unix=time.time(),phase='prepared',root=lease()['remote']+'/cpu-engram-fp8-v73',
            source_sha256=hashlib.sha256(source).hexdigest(),source_bytes=len(source),gpu_executed=False,
            original_source_sha256='f416e336beb9bba744ebd43753172ec1f3dcbc019f19e944a44171fe2d3cb42b')
code='''import base64,hashlib,json,os,shlex,subprocess,time
from pathlib import Path
r=@R@;parent=Path(@P@);archive=Path(@A@);native=Path(@N@);root=Path(r['root'])
if os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or root.exists():raise RuntimeError('exclusive CPU compilation directory differs')
os.umask(0o077);root.mkdir(mode=0o700)
p=Path('/proc/'+str(os.getpid())+'/stat');process={'pid':os.getpid(),'start_ticks':int(p.read_text().split(') ',1)[1].split()[19]),'uid':os.getuid(),'unix':time.time(),'namespace':'jump host'}
(root/'cpu-process.json').write_text(json.dumps(process,indent=2)+chr(10))
state={'phase':'preflight','unix':time.time(),'process':process,'runs':[],'gpu_executed':False,'performance_claimed':False}
def save():
 p=root/'build-result.tmp';p.write_text(json.dumps(state,indent=2)+chr(10));p.replace(root/'build-result.json')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
save()
try:
 object_name='CMakeFiles/ds41_dense.dir/tools/v41/engine/ds41_engram.cpp.o'
 lines=(parent/'build/build.ninja').read_text().splitlines();prefix='build '+object_name+': '
 indices=[i for i,line in enumerate(lines) if line.startswith(prefix)]
 if len(indices)!=1:raise RuntimeError('exact Ninja Engram target differs')
 i=indices[0];rule,source=lines[i].split(': ',1)[1].split(' || ',1)[0].split(' ',1);attributes={}
 for line in lines[i+1:]:
  if not line.startswith('  '):break
  key,value=line.strip().split(' = ',1);attributes[key]=value
 rules=(parent/'build/CMakeFiles/rules.ninja').read_text().splitlines();indices=[i for i,line in enumerate(rules) if line=='rule '+rule]
 if len(indices)!=1:raise RuntimeError('exact C++ rule differs')
 commands=[]
 for line in rules[indices[0]+1:]:
  if not line.startswith('  '):break
  if line.startswith('  command = '):commands.append(line[len('  command = '):])
 if len(commands)!=1:raise RuntimeError('exact compiler template differs')
 command=commands[0]
 for name in ['LAUNCHER','CODE_CHECK']:
  if any(line.startswith(name+' = ') for line in lines+rules):raise RuntimeError('unsupported global compiler wrapper')
  command=command.replace('${'+name+'}',attributes.get(name,''))
 for name in ['DEFINES','INCLUDES','FLAGS','DEP_FILE']:command=command.replace('$'+name,attributes[name])
 command=command.replace('$out',object_name).replace('$in',source)
 if '$' in command:raise RuntimeError('unsupported original compiler substitution')
 args=shlex.split(command)
 if args[0]!='/usr/bin/c++' or args[args.index('-c')+1]!=source:raise RuntimeError('exact original compiler/input differs')
 original=Path(source)
 if sha(original)!=r['original_source_sha256']:raise RuntimeError('exact original Engram source differs')
 obj=subprocess.run(['ar','p',str(archive),'ds41_engram.cpp.o'],capture_output=True,check=True,timeout=5).stdout
 if len(obj)>4*1024**2:raise RuntimeError('bounded original Engram object exceeded')
 expected=hashlib.sha256(obj).hexdigest();state.update(original_archive=str(archive),original_archive_sha256=sha(archive),original_object_sha256=expected,original_compile_command=command)
 clean=[];i=0
 while i<len(args):
  if args[i] in ['-MT','-MF','-o','-c']:i+=2;continue
  if args[i]=='-MD':i+=1;continue
  clean.append(args[i]);i+=1
 raw=base64.b64decode(@DATA@)
 if len(raw)!=r['source_bytes'] or hashlib.sha256(raw).hexdigest()!=r['source_sha256']:raise RuntimeError('frozen LUT source differs')
 (root/'ds41_engram.cpp').write_bytes(raw)
 env={key:value for key,value in os.environ.items() if not key.startswith(('STRATA_','ASTER_XQ_'))};env.update(CUDA_VISIBLE_DEVICES='',OMP_NUM_THREADS='1')
 for name,path,extra in [('original',original,[]),('default0',root/'ds41_engram.cpp',[]),('enabled1',root/'ds41_engram.cpp',['-DASTER_XQ_ENGRAM_FP8_LUT=1'])]:
  out=root/name;out.mkdir(mode=0o700);target=out/'ds41_engram.cpp.o';state.update(phase='compiling',current=name);save()
  cmd=['nice','-n','19',*clean,*extra,'-o',str(target),'-c',str(path)]
  with (root/(name+'-compile.log')).open('xb') as log:p=subprocess.run(cmd,cwd=root,env=env,stdout=log,stderr=log,timeout=24)
  if p.returncode:raise RuntimeError('exact Engram compile failed '+name)
  digest=sha(target);state['runs'].append({'name':name,'command':cmd,'object':{'path':str(target),'bytes':target.stat().st_size,'sha256':digest}})
  if name!='enabled1' and digest!=expected:raise RuntimeError('qualified original/default0 Engram object is not byte-identical')
  save()
 plan=json.loads((native/'native-link-plan.json').read_text())
 (root/'native34f-link-plan.json').write_text(json.dumps(plan,indent=2)+chr(10))
 state.update(phase='compiled',original_object_byte_equal=True,default0_object_byte_equal=True,candidate_object=state['runs'][-1]['object'],native_linked=False,gpu_qualified=False,finished_unix=time.time());save()
except BaseException as error:
 state.update(phase='failed',error=type(error).__name__+': '+str(error),finished_unix=time.time());save();raise
print(json.dumps(state))
'''.replace('@R@',repr(record)).replace('@P@',repr(parent['plan']['parent'])).replace('@A@',repr(old['root']+'/libds41_dense.a')).replace('@N@',repr(native['root'])).replace('@DATA@',repr(base64.b64encode(source).decode()))
compile(code,'exact-Engram-host-compilation-v73','exec')
with intent.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
record['phase']='execution_requested';intent.write_text(json.dumps(record,indent=2)+'\n')
try:
    result=jump(code,100)
    record.update(phase=result['phase'],process=result['process'])
    (BASE/'xolotl-engram-fp8-build-v73-latest.json').write_text(json.dumps(result,indent=2)+'\n')
except BaseException:
    record['phase']='execution_outcome_requires_inspection';intent.write_text(json.dumps(record,indent=2)+'\n');raise
intent.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps({key:result[key] for key in ['phase','original_object_byte_equal','default0_object_byte_equal','candidate_object','native_linked','gpu_qualified']},indent=2))
