"""CPU-only Nsight conversion and numeric event aggregation; keep raw trace/database private."""
import hashlib
import json
import os
import sqlite3
import subprocess
import time
from pathlib import Path

ROOT=Path(__file__).resolve().parent
STATE=dict(unix=time.time(),phase='preflight',gpu_executed=False,raw_profiler_artifacts_published=False)

def save():
    p=ROOT/'statistics-result.tmp';p.write_text(json.dumps(STATE,indent=2)+'\n');p.replace(ROOT/'statistics-result.json')

try:
    os.umask(0o077);save()
    if os.getuid()!=10250 or ROOT.resolve()!=ROOT or ROOT.stat().st_uid!=10250:raise RuntimeError('exact profile statistics owner differs')
    q=json.loads((ROOT/'result.json').read_text())
    if q['phase']!='complete' or not q['engine_reaped'] or not q['native_target_gone']:
        raise RuntimeError('exact native profiler workload must finish/reap first')
    trace=ROOT/'warm-prose.nsys-rep'
    if trace.is_symlink() or trace.stat().st_uid!=10250 or trace.stat().st_size!=q['trace']['bytes'] or hashlib.sha256(trace.read_bytes()).hexdigest()!=q['trace']['sha256']:
        raise RuntimeError('exact immutable trace differs')
    STATE['phase']='converting';save()
    command=['/usr/local/cuda/bin/nsys','stats',
        '--report=cuda_api_sum,cuda_gpu_kern_sum,cuda_gpu_mem_time_sum,cuda_gpu_mem_size_sum',
        '--format=csv','--output='+str(ROOT/'statistics'),str(trace)]
    with (ROOT/'statistics.stdout.log').open('x') as out,(ROOT/'statistics.stderr.log').open('x') as err:
        p=subprocess.run(command,stdout=out,stderr=err,timeout=240,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''))
    STATE['statistics_exit']=p.returncode
    if p.returncode:raise RuntimeError('Nsight statistics conversion failed; inspect bounded exact logs')
    database=ROOT/'warm-prose.sqlite'
    if database.is_symlink() or database.stat().st_uid!=10250 or not 0<database.stat().st_size<=512*1024**2:
        raise RuntimeError('bounded owner-only profiler database differs')
    STATE['phase']='numeric_aggregation';save()
    db=sqlite3.connect('file:'+str(database)+'?mode=ro',uri=True)
    try:
        required={'CUPTI_ACTIVITY_KIND_KERNEL':{'start','end','demangledName'},
                  'CUPTI_ACTIVITY_KIND_MEMCPY':{'start','end','bytes','copyKind'},
                  'CUPTI_ACTIVITY_KIND_RUNTIME':{'start','end','nameId'},'StringIds':{'id','value'}}
        for table,columns in required.items():
            present={r[1] for r in db.execute('PRAGMA table_info('+table+')')}
            if not columns.issubset(present):raise RuntimeError('explicit Nsight schema differs: '+table)
        def grouped(table,column):
            rows=db.execute('SELECT s.value,COUNT(*),SUM(e.end-e.start),MIN(e.end-e.start),MAX(e.end-e.start) FROM '
                +table+' e JOIN StringIds s ON s.id=e.'+column+' GROUP BY s.value ORDER BY SUM(e.end-e.start) DESC LIMIT 32').fetchall()
            if any(not isinstance(r[0],str) or len(r[0])>4096 for r in rows):raise RuntimeError('bounded exact profiler symbol exceeded')
            return [dict(symbol=r[0],count=r[1],duration_sum_ns=r[2],min_ns=r[3],max_ns=r[4]) for r in rows]
        kernels=grouped('CUPTI_ACTIVITY_KIND_KERNEL','demangledName')
        runtimes=grouped('CUPTI_ACTIVITY_KIND_RUNTIME','nameId')
        kcount,ksum=db.execute('SELECT COUNT(*),SUM(end-start) FROM CUPTI_ACTIVITY_KIND_KERNEL').fetchone()
        mcount,msum,mbytes=db.execute('SELECT COUNT(*),SUM(end-start),SUM(bytes) FROM CUPTI_ACTIVITY_KIND_MEMCPY').fetchone()
        rcount,rsum=db.execute('SELECT COUNT(*),SUM(end-start) FROM CUPTI_ACTIVITY_KIND_RUNTIME').fetchone()
        if min(kcount,mcount,rcount)<=0:raise RuntimeError('complete CUDA kernel/memory/runtime trace required')
        intervals=db.execute('SELECT start,end FROM CUPTI_ACTIVITY_KIND_KERNEL UNION ALL SELECT start,end FROM CUPTI_ACTIVITY_KIND_MEMCPY ORDER BY start').fetchall()
        if not 0<len(intervals)<=2000000 or any(a<0 or b<a for a,b in intervals):raise RuntimeError('bounded recorded GPU event interval differs')
        first=intervals[0][0];end=first;busy=0
        for a,b in intervals:
            if a>end:busy+=b-a;end=b
            elif b>end:busy+=b-end;end=b
        if end<=first:raise RuntimeError('nonzero recorded trace span required')
        memory=[dict(copy_kind_code=r[0],count=r[1],duration_sum_ns=r[2],bytes=r[3]) for r in
            db.execute('SELECT copyKind,COUNT(*),SUM(end-start),SUM(bytes) FROM CUPTI_ACTIVITY_KIND_MEMCPY GROUP BY copyKind ORDER BY SUM(end-start) DESC')]
    finally:db.close()
    summary=dict(schema='aster-xolotl-native-critical-path-v123',unix=time.time(),
        binary_sha256=q['plan']['binary_sha256'],context_tokens=q['plan']['context_tokens'],
        prefill_chunk_tokens=q['plan']['prefill_chunk_tokens'],parallel_slots=q['plan']['parallel_slots'],native_info=q['native_info'],gpu_profiled_requests=1,
        prompt_tokens=q['requests'][-1]['prompt_tokens'],completion_tokens=q['requests'][-1]['completion_tokens'],
        pretrace_control_requests=len(q['requests'])-1,all_output_id_sequences_equal=q['all_output_id_sequences_equal'],
        kernel_count=kcount,kernel_duration_sum_ns=ksum,memcpy_count=mcount,memcpy_duration_sum_ns=msum,memcpy_bytes=mbytes,
        runtime_count=rcount,runtime_duration_sum_ns=rsum,recorded_gpu_event_span_ns=end-first,
        recorded_gpu_event_busy_union_ns=busy,recorded_gpu_event_busy_union_fraction=busy/(end-first),
        top_kernels=kernels,top_runtime_calls=runtimes,memory_by_cupti_copy_kind_code=memory,
        diagnostic_only=True,actual1m_input_executed=False,performance_300_claimed=False,
        limits='Trace overhead is included. Summed kernel/memory/runtime durations overlap and cannot be added as wall time. Event union is only recorded GPU intervals, not SM utilization or a hardware speed ceiling. Pre-trace controls are not post-trace performance qualification.')
    encoded=(json.dumps(summary,indent=2)+'\n').encode()
    if len(encoded)>262144:raise RuntimeError('bounded numeric trace summary exceeded')
    with (ROOT/'numeric-summary.json').open('xb') as output:output.write(encoded)
    STATE.update(phase='passed',summary_sha256=hashlib.sha256(encoded).hexdigest(),summary_bytes=len(encoded),
        trace_bytes=trace.stat().st_size,database_bytes=database.stat().st_size,finished_unix=time.time());save()
except BaseException as error:
    STATE.update(phase='failed',error=type(error).__name__+': '+str(error),finished_unix=time.time());save();raise
