"""Derive a current 1M/S512 profiler from the retained direct native protocol worker."""
import hashlib
import json
from xolotl_trial_common import BASE

parent=BASE/'xolotl_nsys_protocol_worker_v2.frozen.py'
source=parent.read_text()

def replace(before,after):
    global source
    if source.count(before)!=1:raise RuntimeError('pinned native profile worker anchor differs')
    source=source.replace(before,after)

begin="    env={k:v for k,v in os.environ.items() if not k.startswith('STRATA_')}"
end="    if PLAN['visible_devices']=='0,1':command+=['--layer-split','20','--split-device','0,1']"
if source.count(begin)!=1 or source.count(end)!=1:raise RuntimeError('pinned full runtime range differs')
start=source.index(begin);stop=source.index(end,start)+len(end)
source=source[:start]+'''    env={k:v for k,v in os.environ.items() if not k.startswith(('STRATA_','ASTER_XQ_','DS41_CHECK_ARENA'))}
    env.update(CUDA_VISIBLE_DEVICES='0',GGML_NO_BACKTRACE='1',OMP_NUM_THREADS='16',OPENBLAS_NUM_THREADS='1')
    env.update(PLAN['engine_env'])
    env.pop('STRATA_API_KEY',None)
    if PLAN['native_args'][PLAN['native_args'].index('--ctx')+1]!='1048576' or PLAN['native_args'][PLAN['native_args'].index('--prefill-chunk')+1]!='512':raise RuntimeError('exact current 1M/S512 native profile differs')
    import hashlib
    if hashlib.sha256(Path(PLAN['binary']).read_bytes()).hexdigest()!=PLAN['binary_sha256']:raise RuntimeError('exact current native binary SHA differs')
    command=[PLAN['binary'],'--serve']+PLAN['native_args']
'''+source[stop:]
replace("'--trace=cuda'","'--trace=cuda,osrt'")
replace("deadline=start+(20 if case['role']=='profiled' else 600)","deadline=start+(120 if case['role']=='profiled' else 180)")
replace("                if not tokens:raise RuntimeError('trial request returned no tokens')",
        "                if len(tokens)!=128:raise RuntimeError('exact profiler/control128 token count differs')")
replace("        RESULT.update(phase='complete',finished_unix=time.time(),engine_exit=child.returncode)",
        "        RESULT.update(phase='complete',finished_unix=time.time(),engine_exit=child.returncode,\n"
        "            all_output_id_sequences_equal=all([t['id'] for t in r['tokens']]==[t['id'] for t in RESULT['requests'][0]['tokens']] for r in RESULT['requests']),\n"
        "            diagnostic_only=True,performance_300_claimed=False,\n"
        "            scope='Current same9a684/1M/S512 direct single-request native critical path; three pre-trace controls then one trace. Not API throughput or full1M-input validation.')")
replace("    if child is not None and child.poll() is None:\n        target=ROOT/'profile-target-process.json'",
        "    if child is not None:\n        target=ROOT/'profile-target-process.json'")
replace("    RESULT['engine_reaped']=child is None or child.poll() is not None",
        "    target=ROOT/'profile-target-process.json';native_gone=True\n"
        "    if target.exists():\n"
        "        rec=json.loads(target.read_text());p=Path('/proc/'+str(rec['pid'])+'/stat')\n"
        "        native_gone=not (p.exists() and p.stat().st_uid==rec['uid'] and int(p.read_text().split(') ',1)[1].split()[19])==rec['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z')\n"
        "    RESULT['native_target_gone']=native_gone\n"
        "    RESULT['engine_reaped']=(child is None or child.poll() is not None) and native_gone\n"
        "    if not native_gone:RESULT.update(phase='failed',error='Exact native profiler target still alive after bounded cleanup')\n"
        "    closed=subprocess.run([NSYS,'shutdown','--session='+SESSION,'--kill=none'],capture_output=True,text=True,timeout=20)\n"
        "    RESULT['named_session_shutdown_exit']=closed.returncode")
compile(source,'xolotl_native_profile_worker_v117.frozen.py','exec')
path=BASE/'xolotl_native_profile_worker_v117.frozen.py'
with path.open('x',encoding='utf-8',newline='\n') as output:output.write(source)
receipt=dict(parent=parent.name,parent_sha256=hashlib.sha256(parent.read_bytes()).hexdigest(),
    candidate=path.name,candidate_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
    current_context=1048576,prefill_chunk=512,credentials_copied=False,profile_executed=False)
with (BASE/'xolotl-native-profile-v117-prepared.json').open('x') as output:output.write(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt))
