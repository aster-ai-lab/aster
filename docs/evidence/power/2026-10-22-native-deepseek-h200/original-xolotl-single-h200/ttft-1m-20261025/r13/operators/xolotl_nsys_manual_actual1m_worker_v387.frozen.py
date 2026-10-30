"""Trace only the fourth actual1M GEN's1024-token decode, with three exact controls; no kernel replay.

Raw reports remain in this private owned runtime. Only explicitly named
numeric counters and public-output correctness are exported to the receipt.
"""
import hashlib
import json
import math
import os
from pathlib import Path
import queue
import re
import signal
import sqlite3
from aster_nsys_decode_window_v321 import decode_window
from aster_nsys_raw_decode_metrics_v386 import decode_raw_metrics
import subprocess
import sys
import threading
import time
from aster_native_resume_v197 import validate_resume
from aster_nsys_lifecycle_v302 import require_no_active_sessions

ROOT=Path(__file__).resolve().parent
P=json.loads((ROOT/'plan.json').read_text())
STATE=dict(unix=time.time(),phase='preflight',requests=[],diagnostic_only=True,hardware_counters_collected=False,
    clock_control='none',cache_control='none',model_math_changed=False,performance_300_claimed=False,
    actual1m_input_executed=False,raw_report_publication_permitted=False)
NSYS='/usr/local/cuda/bin/nsys';SESSION='aster-h200-nsys-actual1m-v387';child=None;pidfd=None;events=queue.Queue(maxsize=8192);capture_active=False

def save():
    raw=(json.dumps(STATE,indent=2)+'\n').encode()
    if len(raw)>2*1024**2:raise RuntimeError('bounded public profiling receipt exceeded')
    p=ROOT/'result.tmp';p.write_bytes(raw);p.replace(ROOT/'result.json')

def sha(path):
    h=hashlib.sha256()
    with path.open('rb') as source:
        while raw:=source.read(4*1024**2):h.update(raw)
    return h.hexdigest()

def terminate(signum,frame):
    raise InterruptedError('finite exact profiler deadline/signal reached')

def reader(pipe):
    try:
        for line in iter(pipe.readline,''):
            if len(line)>65536:raise RuntimeError('bounded native output line exceeded')
            events.put((time.monotonic(),line.rstrip('\n')),timeout=5)
    except BaseException as error:
        events.put((time.monotonic(),'READER_ERROR '+type(error).__name__+': '+str(error)[:500]),timeout=5)
    finally:events.put((time.monotonic(),None),timeout=5)

def profile_tool(action):
    command=[NSYS,action,'--session='+SESSION]
    if action=='start':command+=['--sample=none','--cpuctxsw=none','--capture-range=none',
        '--gpu-metrics-devices='+str(P['nsys_gpu_index']),'--gpu-metrics-set=gh100',
        '--gpu-metrics-frequency=100','--output='+str(ROOT/'private-decode')]
    with (ROOT/('nsys-'+action+'.log')).open('x') as out:
        result=subprocess.run(command,stdout=out,stderr=out,timeout=90)
    if result.returncode:raise RuntimeError('exact named Nsight Systems '+action+' failed')

def exact_alive(record):
    p=Path('/proc/'+str(record['pid'])+'/stat')
    return p.exists() and p.stat().st_uid==record['uid'] and int(p.read_text().split(') ',1)[1].split()[19])==record['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'

try:
    os.umask(0o077);save()
    signal.signal(signal.SIGTERM,terminate);signal.signal(signal.SIGINT,terminate);signal.signal(signal.SIGALRM,terminate);signal.alarm(2570)
    if os.getuid()!=10250 or ROOT.resolve()!=ROOT or ROOT.stat().st_uid!=10250:raise RuntimeError('exact native profiling root/UID differs')
    proof=json.loads((ROOT.parent/'resource-result.json').read_text())
    if proof['phase']!='passed' or len(proof['gpus'])!=1 or proof['gpus'][0]['uuid']!=P['expected_gpu_uuid'] or time.time()+2450>proof['leaseEndsUnix']:
        raise RuntimeError('finite exact actual1M profiling lease/resource differs')
    inventory=subprocess.run(['nvidia-smi','--query-gpu=index,uuid,pci.bus_id','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
    if inventory!=P['NVML_inventory_line'] or P['nsys_gpu_index']!=0:raise RuntimeError('exact explicit Nsight physical-device mapping differs')
    active=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5)
    if active.stdout.strip():raise RuntimeError('exact optimization GPU must be idle before profiler launch')
    for row in P['inputs']:
        p=Path(row['path'])
        if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or sha(p)!=row['sha256']:raise RuntimeError('exact profiling input differs: '+p.name)
    if sha(Path(P['binary']))!=P['binary_sha256']:raise RuntimeError('exact new controlled profiling binary differs')
    if P['native_args'][P['native_args'].index('--ctx')+1]!='1048576' or P['native_args'][P['native_args'].index('--prefill-chunk')+1]!='512' or P['native_args'][P['native_args'].index('--batch')+1]!='2':
        raise RuntimeError('exact actual API1M/S512/native batch2 profile differs')
    env={key:os.environ[key] for key in ['PATH','LD_LIBRARY_PATH'] if key in os.environ}
    env.update(CUDA_VISIBLE_DEVICES='0',GGML_NO_BACKTRACE='1',OMP_NUM_THREADS='16',OPENBLAS_NUM_THREADS='1',LC_ALL='C')
    env.update(P['engine_env']);env['ASTER_XQ_SERVE_PROFILE']='1'
    if 'STRATA_API_KEY' in env:raise RuntimeError('profiler child must have no API credential')
    # Execute the exact CPU-only stdio smoke before any model load, in this now-idle pod.
    smoke_script=ROOT/'stdio-smoke.py'
    smoke_script.write_text("import sys;print('READY_CPU',flush=True);line=sys.stdin.readline();print('ECHO_CPU '+line.strip(),flush=True)\n")
    smoke=subprocess.run([NSYS,'launch','--session-new='+SESSION+'-cpu','--trace=osrt','--wait=primary',
        '--show-output=true',sys.executable,'-u',str(smoke_script)],input='public-profile-pipe-check\n',
        capture_output=True,text=True,env=dict(env,CUDA_VISIBLE_DEVICES=''),timeout=30)
    (ROOT/'cpu-protocol.stdout.log').write_text(smoke.stdout);(ROOT/'cpu-protocol.stderr.log').write_text(smoke.stderr)
    expected=['READY_CPU','ECHO_CPU public-profile-pipe-check']
    lines=smoke.stdout.splitlines()
    if smoke.returncode or [line for line in lines if line.startswith(('READY_CPU','ECHO_CPU'))]!=expected:
        raise RuntimeError('installed nsys launch bidirectional CPU pipe smoke failed')
    sessions=subprocess.run([NSYS,'sessions','list','--show-header=false'],capture_output=True,text=True,timeout=8)
    (ROOT/'cpu-sessions.stdout.log').write_text(sessions.stdout);(ROOT/'cpu-sessions.stderr.log').write_text(sessions.stderr)
    require_no_active_sessions(sessions)
    STATE.update(nsys_CPU_pipe_smoke_passed=True,nsys_CPU_session_absent_after_primary_exit=True);save()
    target=ROOT/'native-target.py'
    target.write_text("import os,sys,json\nfrom pathlib import Path\nr=Path(__file__).resolve().parent\nb=sys.argv[1]\np=dict(pid=os.getpid(),start_ticks=int(Path('/proc/self/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),binary=b,namespace='optimization pod')\n(r/'native-target-process.json').write_text(json.dumps(p))\nos.execv(b,sys.argv[1:])\n")
    command=[NSYS,'launch','--session-new='+SESSION,'--trace=cuda','--cuda-graph-trace=node',
        '--wait=primary','--show-output=true',sys.executable,'-u',str(target),P['binary'],'--serve',*P['native_args']]
    STATE.update(phase='loading',command=command,profiler_launched_unix=time.time());save()
    with (ROOT/'engine.stderr.log').open('xb') as stderr:
        child=subprocess.Popen(command,cwd=ROOT,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=stderr,text=True,bufsize=1,start_new_session=True)
        pidfd=os.pidfd_open(child.pid)
        handle=dict(pid=child.pid,start_ticks=int(Path('/proc/'+str(child.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),namespace='optimization pod')
        (ROOT/'nsys-launch-process.json').write_text(json.dumps(handle,indent=2)+'\n')
        threading.Thread(target=reader,args=(child.stdout,),daemon=True).start()
        deadline=time.monotonic()+150
        while True:
            ts,line=events.get(timeout=max(.01,deadline-time.monotonic()))
            if line is None or line.startswith(('ERR ','READER_ERROR ')):raise RuntimeError('native profiler target failed before READY: '+str(line))
            if line.startswith('INFO '):
                fields=line.split()[1:]
                if any('=' not in value for value in fields):raise RuntimeError('exact native INFO syntax differs')
                info=dict(value.split('=',1) for value in fields)
                if len(info)!=len(fields) or info.get('ctx')!='1048576' or info.get('batch_slots')!='2' or info.get('slot_cache')!='1':raise RuntimeError('exact native1M/batch2 INFO differs')
                STATE['native_info']=info
            if line.startswith('READY '):
                if not STATE.get('native_info'):raise RuntimeError('exact native INFO proof missing')
                break
        STATE.update(phase='running',ready_unix=time.time());save()
        source=Path(P['input_root']);meta=json.loads((source/'input-result.json').read_text())
        csv_bytes=(source/'prompt.csv').read_bytes();binary=(source/'prompt.u32le').read_bytes()
        if meta['phase']!='passed' or meta['prompt_tokens']!=1047544 or hashlib.sha256(binary).hexdigest()!=meta['prompt_id_sha256']:
            raise RuntimeError('exact full1M protected API input IDs differ')
        input_csv=csv_bytes.decode('ascii')
        if len(csv_bytes)>8*1024**2 or input_csv.count(',')+1!=1047544:raise RuntimeError('bounded actual1M native protocol token count differs')
        with (ROOT/'public-protocol.jsonl').open('x') as journal:
            for index in range(4):
                role='profiled' if index==3 else 'unprofiled_control'
                STATE.update(current_request=index,current_role=role,current_state='prefilling');save()
                if index==3:profile_tool('start');capture_active=True
                start=time.monotonic();tokens=[];metadata=[]
                child.stdin.write('GEN 1024 temperature=0 seed=1 '+input_csv+'\n');child.stdin.flush()
                deadline=start+(2400 if index==0 else 180 if index==3 else 300)
                count=0
                while True:
                    ts,line=events.get(timeout=max(.01,deadline-time.monotonic()))
                    if line is None or line.startswith(('ERR ','READER_ERROR ')):raise RuntimeError('native actual1M profile request failed: '+str(line))
                    journal.write(json.dumps(dict(request=index,role=role,seconds=ts-start,line=line))+'\n');count+=1
                    if count>10000:raise RuntimeError('bounded profiling protocol output exceeded')
                    if line.startswith('T '):
                        values=line.split()
                        if len(values)!=2:raise RuntimeError('exact native token protocol differs')
                        tokens.append(dict(id=int(values[1]),seconds=ts-start))
                        if len(tokens)==1:STATE.update(current_state='streaming',current_first_text_unix=time.time());save()
                        if len(tokens)>1024:raise RuntimeError('profile diagnostic emitted more than1024 tokens')
                    elif line.startswith('DONE '):break
                    else:metadata.append(line)
                done=line.split()
                if len(done)<9 or int(done[1])!=1024 or int(done[2])!=1047544 or len(tokens)!=1024 or done[5]!='length':
                    raise RuntimeError('actual1M/1024-output native completion differs')
                validate_resume(metadata,line,index,output_tokens=1024)
                if capture_active:profile_tool('stop');capture_active=False
                elapsed=time.monotonic()-start;span=tokens[-1]['seconds']-tokens[0]['seconds']
                STATE['requests'].append(dict(index=index,role=role,prompt_tokens=1047544,completion_tokens=1024,tokens=tokens,
                    ttft_seconds=tokens[0]['seconds'],request_seconds=elapsed,selected_ID_interval_tok_s=1023/span if span>0 else None,
                    done=line,protocol_metadata=metadata));save()
        child.stdin.write('QUIT\n');child.stdin.flush();child.stdin.close();child.wait(timeout=90)
        if child.returncode:raise RuntimeError('nsys/native target exited unsuccessfully')
    reference=[token['id'] for token in STATE['requests'][0]['tokens']]
    if any([token['id'] for token in row['tokens']]!=reference for row in STATE['requests']):raise RuntimeError('profiling changed complete output ID sequence')
    source=Path(P['public_api_root'])
    sys.path[:0]=[str(source/'python-deps-preflight'),str(source/'source'),str(source/'source/tools/v41/serve')]
    import serve_ds41 as D
    vocab=json.loads((source/'tokenizer/vocab.json').read_text());values=[None]*len(vocab)
    for word,index in vocab.items():values[index]=word
    tokenizer=D.Ds41Tokenizer(values,(source/'tokenizer/merges.txt').read_text().split('\n'),json.loads((source/'tokenizer/token_type.json').read_text()))
    from xolotl_fullwindow_long_fixture_v54 import check_record_prefix
    output_text=tokenizer.decode(reference,errors='replace');check_record_prefix(output_text)
    output_hash=hashlib.sha256(output_text.encode()).hexdigest()
    STATE.update(public_output_sha256=output_hash,API_canonical_output_qualified=output_hash=='7f9b16debd8c73a488bdf1ef1ca517afee6c9e27581e6ca2964b7ef86422e7cf');save()
    report=ROOT/'private-decode.nsys-rep'
    if report.is_symlink() or report.stat().st_uid!=10250 or not 0<report.stat().st_size<=512*1024**2:
        raise RuntimeError('bounded private actual1M trace absent or differs')
    log=(ROOT/'engine.stderr.log').read_text(errors='replace')
    if log.count('aster serve profile: fourth_GEN_decode_begin')!=1 or log.count('aster serve profile: fourth_GEN_decode_end')!=1:
        raise RuntimeError('exact fourth-only native profiling window not established')
    database=ROOT/'private-decode.sqlite'
    with (ROOT/'nsys-export.log').open('x') as out:
        exported=subprocess.run([NSYS,'export','--type=sqlite','--output='+str(database),str(report)],stdout=out,stderr=out,timeout=90)
    if exported.returncode or database.is_symlink() or database.stat().st_uid!=10250 or not 0<database.stat().st_size<=1024**3:raise RuntimeError('bounded exact private decode SQLite export failed')
    db=sqlite3.connect(database.as_uri()+'?mode=ro&immutable=1',uri=True);db.execute('PRAGMA query_only=ON')
    window=decode_window(db);metrics=decode_raw_metrics(db,window);db.close()
    STATE.update(decode_window=window,decode_metrics=metrics,manual_capture_export_passed=True,hardware_counters_collected=True,physical_counter_values_qualified=False,
        hardware_scope='fourth actual1M/1024 native GEN; marker-filtered decode only; debug trace does not qualify API output or speed',database_bytes=database.stat().st_size);save()
    STATE.update(phase='complete',actual1m_input_executed=True,actual_profiled_prompt_tokens=1047544,
        actual_profiled_output_tokens=1024,all_output_ID_sequences_equal=True,three_position_retrieval_passed=True,
        public_output_sha256=output_hash,hardware_counters_collected=True,numeric_interpretation_pending=False,
        report=dict(bytes=report.stat().st_size,sha256=sha(report),publication_permitted=False),
        input_ID_sha256=meta['prompt_id_sha256'],input_CSV_sha256=hashlib.sha256(csv_bytes).hexdigest(),
        native_internal_input_SHA256_captured=False,finished_unix=time.time(),GPU_kernel_replay_used=False,
        capture='manual capture fourthGEN; explicit GPU index0; exact successful CUDA start/stop markers filter decode after export',
        scope='Actual1047544 protected API input IDs and complete1024 output. Three unprofiled controls, then fourth-GEN decode-only CUDA timeline and100Hz exact-device HWmetrics. Diagnostic overhead included; no API throughput, performance acceptance or hardware speed ceiling claimed.')
    save()
    if not STATE['API_canonical_output_qualified']:
        raise RuntimeError('complete1024 diagnostic text differs from qualified canonical reference; retained hardware is debug-only')
except BaseException as error:
    STATE.update(phase='failed',error=type(error).__name__+': '+str(error)[:2500],finished_unix=time.time());save();raise
finally:
    signal.alarm(0)
    if capture_active:
        try:profile_tool('stop');capture_active=False
        except BaseException as error:STATE['capture_stop_error']=type(error).__name__+': '+str(error)[:1000]
    record_path=ROOT/'native-target-process.json'
    if record_path.exists():
        record=json.loads(record_path.read_text())
        if exact_alive(record):
            descriptor=os.pidfd_open(record['pid'])
            try:
                if exact_alive(record):signal.pidfd_send_signal(descriptor,signal.SIGTERM)
            except ProcessLookupError:pass
            finally:os.close(descriptor)
            deadline=time.monotonic()+10
            while exact_alive(record) and time.monotonic()<deadline:time.sleep(.2)
            if exact_alive(record):
                descriptor=os.pidfd_open(record['pid'])
                try:
                    if exact_alive(record):signal.pidfd_send_signal(descriptor,signal.SIGKILL)
                except ProcessLookupError:pass
                finally:os.close(descriptor)
    if child is not None and child.poll() is None:
        if pidfd is not None:signal.pidfd_send_signal(pidfd,signal.SIGTERM)
        try:child.wait(timeout=10)
        except subprocess.TimeoutExpired:
            signal.pidfd_send_signal(pidfd,signal.SIGKILL);child.wait(timeout=10)
    if pidfd is not None:os.close(pidfd)
    native_gone=not record_path.exists() or not exact_alive(json.loads(record_path.read_text()))
    STATE.update(native_target_gone=native_gone,engine_reaped=(child is None or child.poll() is not None) and native_gone)
    if not native_gone:STATE.update(phase='failed',error='exact native profiler target survived bounded cleanup')
    sessions=subprocess.run([NSYS,'sessions','list','--show-header=false'],capture_output=True,text=True,timeout=8)
    if sessions.returncode==0 and sessions.stdout.strip():
        closed=subprocess.run([NSYS,'shutdown','--session='+SESSION,'--kill=none'],capture_output=True,text=True,timeout=20)
        STATE['named_session_shutdown_exit']=closed.returncode
        (ROOT/'session-cleanup.stdout.log').write_text(closed.stdout);(ROOT/'session-cleanup.stderr.log').write_text(closed.stderr)
        sessions=subprocess.run([NSYS,'sessions','list','--show-header=false'],capture_output=True,text=True,timeout=8)
    try:
        require_no_active_sessions(sessions);STATE['named_sessions_closed']=True
    except ValueError as error:
        STATE.update(phase='failed',named_sessions_closed=False,error=str(error))
    save()
