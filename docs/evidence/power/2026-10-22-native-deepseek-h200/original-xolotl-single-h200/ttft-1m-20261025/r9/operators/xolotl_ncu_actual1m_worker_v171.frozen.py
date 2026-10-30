"""Profile only the fourth actual1M GEN's decode; keep three exact unprofiled controls.

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
import subprocess
import sys
import threading
import time
from aster_ncu_metrics_v171 import METRICS, FAMILIES, numeric_rows, family_counts

ROOT=Path(__file__).resolve().parent
P=json.loads((ROOT/'plan.json').read_text())
STATE=dict(unix=time.time(),phase='preflight',requests=[],diagnostic_only=True,hardware_counters_collected=False,
    clock_control='none',cache_control='none',model_math_changed=False,performance_300_claimed=False,
    actual1m_input_executed=False,raw_report_publication_permitted=False)
NCU='/usr/local/bin/ncu';child=None;pidfd=None;events=queue.Queue(maxsize=8192)

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

def exact_alive(record):
    p=Path('/proc/'+str(record['pid'])+'/stat')
    return p.exists() and p.stat().st_uid==record['uid'] and int(p.read_text().split(') ',1)[1].split()[19])==record['start_ticks'] and p.read_text().split(') ',1)[1].split()[0]!='Z'

try:
    os.umask(0o077);save()
    signal.signal(signal.SIGTERM,terminate);signal.signal(signal.SIGINT,terminate);signal.signal(signal.SIGALRM,terminate);signal.alarm(4470)
    if os.getuid()!=10250 or ROOT.resolve()!=ROOT or ROOT.stat().st_uid!=10250:raise RuntimeError('exact native profiling root/UID differs')
    proof=json.loads((ROOT.parent/'resource-result.json').read_text())
    if proof['phase']!='passed' or len(proof['gpus'])!=1 or proof['gpus'][0]['uuid']!=P['expected_gpu_uuid'] or time.time()+4350>proof['leaseEndsUnix']:
        raise RuntimeError('finite exact actual1M profiling lease/resource differs')
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
    # Validate bidirectional pipe behavior before paying for model load/prefill.
    cpu_code="import sys;print('READY_CPU',flush=True);line=sys.stdin.readline();print('ECHO_CPU '+line.strip(),flush=True)"
    smoke=subprocess.run([NCU,'--profile-from-start','off','--clock-control','none','--cache-control','none',
        '--log-file',str(ROOT/'cpu-protocol-ncu.log'),sys.executable,'-u','-c',cpu_code],
        input='public-profile-pipe-check\n',capture_output=True,text=True,env=env,timeout=30)
    (ROOT/'cpu-protocol.stdout.log').write_text(smoke.stdout);(ROOT/'cpu-protocol.stderr.log').write_text(smoke.stderr)
    if smoke.returncode or smoke.stdout!='READY_CPU\nECHO_CPU public-profile-pipe-check\n':raise RuntimeError('installed ncu bidirectional CPU pipe smoke failed')
    STATE['ncu_CPU_pipe_smoke_passed']=True;save()
    bases=','.join(dict.fromkeys(metric.split('.',1)[0] for metric in METRICS))
    query=subprocess.run([NCU,'--query-metrics-mode','all','--metrics',bases],capture_output=True,text=True,env=env,timeout=30)
    (ROOT/'metric-query.stdout.log').write_text(query.stdout);(ROOT/'metric-query.stderr.log').write_text(query.stderr)
    if query.returncode or len(query.stdout)>1048576 or any(metric not in query.stdout for metric in METRICS):
        raise RuntimeError('installed H200 profiler query did not confirm all exact requested metrics')
    STATE['exact_metrics_confirmed']=list(METRICS);save()
    target=ROOT/'native-target.py'
    target.write_text("import os,sys,json\nfrom pathlib import Path\nr=Path(__file__).resolve().parent\nb=sys.argv[1]\np=dict(pid=os.getpid(),start_ticks=int(Path('/proc/self/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),binary=b,namespace='optimization pod')\n(r/'native-target-process.json').write_text(json.dumps(p))\nos.execv(b,sys.argv[1:])\n")
    # Each literal CUDA identifier is taken from the completed current native trace.
    expression='regex:(?:^|::|\\s)(?:'+'|'.join(re.escape(name) for name in FAMILIES)+')(?:<|\\()'
    command=[NCU,'--profile-from-start','off','--replay-mode','kernel','--graph-profiling','node',
        '--clock-control','none','--cache-control','none','--target-processes','all',
        '--kernel-name-base','demangled','--kernel-name',expression,'--launch-count','24',
        '--metrics',','.join(METRICS),'--log-file',str(ROOT/'private-ncu.log'),'--export',str(ROOT/'decode'),
        sys.executable,'-u',str(target),P['binary'],'--serve',*P['native_args']]
    STATE.update(phase='loading',command=command,profiler_launched_unix=time.time());save()
    with (ROOT/'engine.stderr.log').open('xb') as stderr:
        child=subprocess.Popen(command,cwd=ROOT,env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=stderr,text=True,bufsize=1,start_new_session=True)
        pidfd=os.pidfd_open(child.pid)
        handle=dict(pid=child.pid,start_ticks=int(Path('/proc/'+str(child.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),namespace='optimization pod')
        (ROOT/'ncu-launch-process.json').write_text(json.dumps(handle,indent=2)+'\n')
        threading.Thread(target=reader,args=(child.stdout,),daemon=True).start()
        deadline=time.monotonic()+600
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
                start=time.monotonic();tokens=[];metadata=[]
                child.stdin.write('GEN 128 temperature=0 seed=1 '+input_csv+'\n');child.stdin.flush()
                deadline=start+(3600 if index==0 else 900 if index==3 else 300)
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
                        if len(tokens)>128:raise RuntimeError('profile diagnostic emitted more than128 tokens')
                    elif line.startswith('DONE '):break
                    else:metadata.append(line)
                done=line.split()
                if len(done)<9 or int(done[1])!=128 or int(done[2])!=1047544 or len(tokens)!=128 or done[5]!='length':
                    raise RuntimeError('actual1M/128-output native completion differs')
                resumes=[int(value.split()[1]) for value in metadata if value.startswith('RESUME ')]
                if len(resumes)!=1 or resumes[0]!=(0 if index==0 else 1047544):raise RuntimeError('exact cold/three restored native prefixes differ')
                elapsed=time.monotonic()-start;span=tokens[-1]['seconds']-tokens[0]['seconds']
                STATE['requests'].append(dict(index=index,role=role,prompt_tokens=1047544,completion_tokens=128,tokens=tokens,
                    ttft_seconds=tokens[0]['seconds'],request_seconds=elapsed,selected_ID_interval_tok_s=127/span if span>0 else None,
                    done=line,protocol_metadata=metadata));save()
        child.stdin.write('QUIT\n');child.stdin.flush();child.stdin.close();child.wait(timeout=90)
        if child.returncode:raise RuntimeError('ncu/native target exited unsuccessfully')
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
    report=ROOT/'decode.ncu-rep'
    if report.is_symlink() or report.stat().st_uid!=10250 or not 0<report.stat().st_size<=128*1024**2:raise RuntimeError('bounded private hardware report absent or differs')
    imported=subprocess.run([NCU,'--import',str(report),'--csv','--page','raw','--print-units','base','--print-fp'],
        env=dict(env,CUDA_VISIBLE_DEVICES=''),capture_output=True,text=True,timeout=90)
    (ROOT/'private-raw-metrics.csv').write_text(imported.stdout);(ROOT/'metrics-import.stderr.log').write_text(imported.stderr)
    if imported.returncode:raise RuntimeError('private hardware counter report import failed')
    rows=numeric_rows(imported.stdout);counts=family_counts(rows)
    coverage=bool(counts['native_gu_multi_kernel']+counts['native_gu_amd_kernel']) and bool(counts['native_down_multi_kernel']+counts['native_down_amd_kernel']) and counts['mul_mat_vec_q']>0 and counts['flash_attn_ext_f16']>0
    log=(ROOT/'engine.stderr.log').read_text(errors='replace')
    if log.count('aster serve profile: fourth_GEN_decode_begin')!=1 or log.count('aster serve profile: fourth_GEN_decode_end')!=1:raise RuntimeError('exact fourth-only native profiling window not established')
    STATE.update(phase='complete',actual1m_input_executed=True,actual_profiled_prompt_tokens=1047544,
        actual_profiled_output_tokens=128,all_output_ID_sequences_equal=True,three_position_retrieval_passed=True,
        public_output_sha256=hashlib.sha256(output_text.encode()).hexdigest(),hardware_counters_collected=True,
        numeric_metrics=rows,kernel_family_counts=counts,all_four_hot_groups_captured=bool(coverage),
        report=dict(bytes=report.stat().st_size,sha256=sha(report),publication_permitted=False),
        input_ID_sha256=meta['prompt_id_sha256'],input_CSV_sha256=hashlib.sha256(csv_bytes).hexdigest(),
        native_internal_input_SHA256_captured=False,finished_unix=time.time(),
        scope='Actual1047544 protected API input IDs,128-output diagnostic, native1M/S512/batch2. Three unprofiled controls then fourth-GEN decode-only sample of24 declared hot kernels. Replay/cache effects are diagnostic; not API throughput, complete1024-output acceptance or a hardware speed ceiling.')
    save()
except BaseException as error:
    STATE.update(phase='failed',error=type(error).__name__+': '+str(error)[:2500],finished_unix=time.time());save();raise
finally:
    signal.alarm(0)
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
    save()
