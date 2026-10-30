import hashlib,json,math,os,statistics,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parent;P=json.loads((ROOT/'plan.json').read_text())
S=dict(unix=time.time(),phase='preflight',profiles=[],model_executed=False,model_speed_claimed=False,formal_changed=False)
def save():
 p=ROOT/'result.tmp';p.write_text(json.dumps(S,indent=2)+'\n');p.replace(ROOT/'result.json')
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  while b:=f.read(4*1024**2):h.update(b)
 return h.hexdigest()
def run(args,name,timeout,env,cwd=None):
 S['phase']=name;save()
 with (ROOT/(name+'.stdout.log')).open('x') as out,(ROOT/(name+'.stderr.log')).open('x') as err:
  q=subprocess.run(args,env=env,cwd=cwd or ROOT,stdin=subprocess.DEVNULL,stdout=out,stderr=err,timeout=timeout)
 if q.returncode:raise RuntimeError(name+' exit '+str(q.returncode))
 a=ROOT/(name+'.stdout.log');b=ROOT/(name+'.stderr.log')
 if a.stat().st_size+b.stat().st_size>262144:raise RuntimeError('bounded operator benchmark log exceeded')
 return a.read_text()
try:
 os.umask(0o077);os.environ['CUDA_VISIBLE_DEVICES']='';save()
 proof=json.loads((ROOT.parent/'resource-result.json').read_text())
 if ROOT.resolve()!=ROOT or ROOT.stat().st_uid!=10250 or proof['phase']!='passed' or proof['gpus'][0]['uuid']!=P['gpu_uuid'] or time.time()+650>proof['leaseEndsUnix']:raise RuntimeError('exact million-row benchmark owner/lease differs')
 qualified=json.loads((Path(P['qualified_root'])/'result.json').read_text())
 if qualified['phase']!='passed' or not qualified['full_score_byte_identity_passed'] or not qualified['memcheck_zero_errors']:raise RuntimeError('qualified candidate/replay/memcheck gates required')
 for row in P['inputs']:
  p=Path(row['path'])
  if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or sha(p)!=row['sha256']:raise RuntimeError('frozen benchmark input differs')
 a=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
 if a:raise RuntimeError('million-row benchmark GPU must be idle')
 cpu=dict(os.environ,CUDA_VISIBLE_DEVICES='');run(P['compile'],'benchmark-driver-compile',30,cpu);run(P['link'],'benchmark-link',40,cpu,P['link_cwd'])
 S['same_binary_sha256']=sha(ROOT/'benchmark');save()
 env={k:v for k,v in os.environ.items() if not k.startswith(('STRATA_','ASTER_XQ_','DS41_','MIMO_','GGML_CUDA_'))}
 env.update(CUDA_VISIBLE_DEVICES='0',GGML_NO_BACKTRACE='1',OMP_NUM_THREADS='16',OPENBLAS_NUM_THREADS='1',ASTER_XQ_GRAPH_SOURCE_TYPES='1',ASTER_XQ_INDEXER_MASK_SKIP='1')
 reference=None
 for name,flag in [('control-a','0'),('MXFP4-header','1'),('control-b','0')]:
  path=ROOT/(name+'.scores.bin');t=time.monotonic()
  out=run([str(ROOT/'benchmark'),str(path)],name,180,dict(env,ASTER_XQ_INDEXER_MXFP4_HEADER=flag))
  lines=out.strip().splitlines()
  if len(lines)!=13 or lines[-1]!='PASS million-row operator benchmark cases: 12':raise RuntimeError('all12 million-row benchmark cases required')
  rows=[json.loads(line) for line in lines[:-1]]
  for i,row in enumerate(rows):
   if row['case']!=i or row['rows']!=1048576 or row['heads'] not in [32,64] or row['queries'] not in [1,6] or row['masked_percent'] not in [0,50,90]:raise RuntimeError('closed million-row benchmark shape differs')
   values=row['wall_ms_per_graph']
   if len(values)!=5 or not all(type(v) in [int,float] and math.isfinite(v) and v>0 for v in values):raise RuntimeError('finite five-block operator measurements required')
   row['median_wall_ms']=statistics.median(values)
  if path.is_symlink() or not 0<path.stat().st_size<512*1024**2:raise RuntimeError('complete private score output bound exceeded')
  digest=sha(path)
  if reference is None:reference=digest
  S['profiles'].append(dict(name=name,flag=flag,seconds=time.monotonic()-t,score_bytes=path.stat().st_size,full_score_sha256=digest,full_score_equal=digest==reference,cases=rows));save()
  if digest!=reference:raise RuntimeError('million-row candidate full-score identity failed')
 a,b,c=S['profiles'];comparisons=[]
 for aa,bb,cc in zip(a['cases'],b['cases'],c['cases']):
  comparisons.append(dict(heads=aa['heads'],queries=aa['queries'],masked_percent=aa['masked_percent'],control_a_median_ms=aa['median_wall_ms'],candidate_median_ms=bb['median_wall_ms'],control_b_median_ms=cc['median_wall_ms'],
   candidate_graphs_per_second_gain_vs_a_percent=(aa['median_wall_ms']/bb['median_wall_ms']-1)*100,candidate_graphs_per_second_gain_vs_b_percent=(cc['median_wall_ms']/bb['median_wall_ms']-1)*100))
 S.update(phase='passed',same_GPU_ABA_completed=True,all12_full_scores_identical=True,comparisons=comparisons,
   scope='Synthetic but exact deployed MXFP4 operator shapes:1048576 rows,heads32/64,queries1/6,masks0/50/90percent. CPU wall per logical CUDA graph including launch/guard overhead; no model tokens, no GPU-only kernel time and no model throughput claim.',finished_unix=time.time())
except BaseException as e:S.update(phase='failed',error=type(e).__name__+': '+str(e),finished_unix=time.time());raise
finally:
 a=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip();S['GPU_idle_after']=not a;save()
