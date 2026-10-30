"""Exact native control relink, then override only the qualified guarded CUDA core."""
import hashlib,json,os,subprocess,time
from pathlib import Path
ROOT=Path(__file__).resolve().parent;P=json.loads((ROOT/'plan.json').read_text())
S=dict(unix=time.time(),phase='preflight',GPU_executed=False,model_executed=False,formal_service_changed=False,runs=[])
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def save():
 t=ROOT/'result.tmp';t.write_text(json.dumps(S,indent=2)+'\n');t.replace(ROOT/'result.json')
def run(cmd,name,env,cwd=None,seconds=45):
 with (ROOT/(name+'.log')).open('xb') as log:
  start=time.monotonic();r=subprocess.run(cmd,env=env,cwd=cwd or ROOT,stdin=subprocess.DEVNULL,stdout=log,stderr=log,timeout=seconds)
 S['runs'].append(dict(name=name,returncode=r.returncode,seconds=time.monotonic()-start));save()
 if r.returncode:raise RuntimeError(name+' failed')
try:
 os.umask(0o077);save()
 resource=json.loads((ROOT.parent/'resource-result.json').read_text())
 if os.getuid()!=10250 or ROOT.resolve()!=ROOT or ROOT.stat().st_uid!=10250 or resource['phase']!='passed' or resource['gpus'][0]['uuid']!=P['gpu_uuid'] or time.time()+300>resource['leaseEndsUnix']:raise RuntimeError('exact C15 link resource differs')
 env={k:os.environ[k] for k in ['PATH','LD_LIBRARY_PATH'] if k in os.environ};env.update(CUDA_VISIBLE_DEVICES='',OMP_NUM_THREADS='1')
 if subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip():raise RuntimeError('CPU relink requires idle optimization card')
 for row in P['inputs']:
  p=Path(row['path'])
  if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or sha(p)!=row['sha256']:raise RuntimeError('exact link input differs: '+str(p))
 run(['/usr/bin/c++','-O3','-DNDEBUG','-std=c++17','-c',P['mode_source'],'-o',str(ROOT/'source-type-mode.o')],'mode-compile',env)
 links=[]
 for name in ['control','candidate']:
  command=list(P['native']['control_link']);command[3:3]=[P['bridge']['path'],P['prototype']['path']]
  if name=='candidate':
   marker='ggml/src/ggml-cuda/libggml-cuda.a'
   if command.count(marker)!=1:raise RuntimeError('unique CUDA archive override required')
   i=command.index(marker);command[i:i]=[P['candidate_core']['path'],str(ROOT/'source-type-mode.o')]
  target=ROOT/('ds41_generate.'+name);command[command.index('-o')+1]=str(target)
  S.update(phase='linking',profile=name);save()
  run(command,'link-'+name,env,P['native']['link_cwd'],90)
  row=dict(path=str(target),bytes=target.stat().st_size,sha256=sha(target));links.append(row)
  if name=='control' and row['sha256']!=P['control_sha256']:raise RuntimeError('native 2bc83 unchanged relink must be byte-exact')
 for value in ['', '2','-1','true','01',' 1','0 ','NaN']:
  q=subprocess.run([links[1]['path'],'--help'],env=dict(env,ASTER_XQ_GRAPH_SOURCE_TYPES=value,ASTER_XQ_TOPK35='1'),capture_output=True,text=True,timeout=5)
  if q.returncode!=2 or q.stderr.strip()!='ASTER_XQ_GRAPH_SOURCE_TYPES must be 0 or 1':raise RuntimeError('native graph-type invalid mode must refuse before model load')
 if subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip():raise RuntimeError('CPU native link left GPU work')
 S.update(phase='linked',control=links[0],candidate=links[1],mode_object=dict(path=str(ROOT/'source-type-mode.o'),sha256=sha(ROOT/'source-type-mode.o')),control_byte_equal=True,
  original_indexer_unchanged=True,MXFP4_header_candidate_excluded=True,only_CUDA_core_replaced=True,invalid_modes_refused=8,model_quality_qualified=False,finished_unix=time.time());save()
except BaseException as e:S.update(phase='failed',error=type(e).__name__+': '+str(e),finished_unix=time.time());save();raise
