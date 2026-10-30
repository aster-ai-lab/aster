"""Controlled original CUDA-core compile and source-type guard qualification, without model inference."""
import hashlib,json,os,struct,subprocess,time
from aster_elf_file_control_v344 import normalize_file_symbol
from pathlib import Path
ROOT=Path(__file__).resolve().parent;P=json.loads((ROOT/'plan.json').read_text())
S=dict(unix=time.time(),phase='preflight',runs=[],comparisons=[],type_switch_tests=[],model_executed=False,performance_claimed=False,formal_changed=False)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save():
 p=ROOT/'result.tmp';p.write_text(json.dumps(S,indent=2)+'\n');p.replace(ROOT/'result.json')
def run(args,name,seconds,env,expected=0,cwd=None):
 S['phase']=name;save()
 with (ROOT/(name+'.stdout.log')).open('x') as out,(ROOT/(name+'.stderr.log')).open('x') as err:
  t=time.monotonic();q=subprocess.run(args,env=env,cwd=cwd or ROOT,stdin=subprocess.DEVNULL,stdout=out,stderr=err,timeout=seconds)
 S['runs'].append(dict(name=name,returncode=q.returncode,seconds=time.monotonic()-t));save()
 if q.returncode!=expected:raise RuntimeError(name+' exit '+str(q.returncode)+' expected '+str(expected))
 a=ROOT/(name+'.stdout.log');b=ROOT/(name+'.stderr.log')
 if a.stat().st_size+b.stat().st_size>2*1024**2:raise RuntimeError('bounded guard logs exceeded')
 return a.read_text(),b.read_text()
def score(p,cases):
 if p.is_symlink() or p.stat().st_size>4*1024**2:raise RuntimeError('bounded full score output exceeded')
 raw=p.read_bytes();offset=0;ncase=0
 while offset<len(raw):
  if offset+4>len(raw):raise RuntimeError('truncated score header')
  n=struct.unpack_from('<I',raw,offset)[0];offset+=4
  if n!=1542 or offset+4*n>len(raw):raise RuntimeError('complete actual score shape differs')
  offset+=4*n;ncase+=1
 if ncase!=cases:raise RuntimeError('complete graph score case count differs')
 return raw
try:
 os.umask(0o077);os.environ['CUDA_VISIBLE_DEVICES']='';save()
 proof=json.loads((ROOT.parent/'resource-result.json').read_text())
 if ROOT.resolve()!=ROOT or ROOT.stat().st_uid!=10250 or proof['phase']!='passed' or proof['gpus'][0]['uuid']!=P['gpu_uuid'] or time.time()+930>proof['leaseEndsUnix']:raise RuntimeError('exact core guard resource/lease differs')
 for row in P['inputs']:
  p=Path(row['path'])
  if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size!=row['bytes'] or sha(p)!=row['sha256']:raise RuntimeError('frozen guard input differs: '+str(p))
 a=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
 if a:raise RuntimeError('guard build requires idle optimization GPU')
 cpu=dict(os.environ,CUDA_VISIBLE_DEVICES='')
 run(P['original_compile'],'original-core-control-compile',240,cpu,cwd=P['compile_cwd'])
 actual=sha(ROOT/'core-control.o');S.update(control_object_sha256=actual,original_core_byte_equal=actual==P['original_object_sha256']);save()
 if not S['original_core_byte_equal']:
  original=subprocess.run(['ar','p',P['original_archive'],P['original_archive_member']],capture_output=True,check=True,timeout=8).stdout
  if hashlib.sha256(original).hexdigest()!=P['original_object_sha256']:raise RuntimeError('qualified original CUDA archive member changed')
  normalized,bookkeeping=normalize_file_symbol(original,(ROOT/'core-control.o').read_bytes())
  with (ROOT/'core-control.STT_FILE-normalized.o').open('xb') as f:f.write(normalized)
  S.update(original_core_bookkeeping=bookkeeping,original_core_equal_after_filename_normalization=True)
  save()
 else:S['original_core_equal_after_filename_normalization']=True

 for name,args in P['compiles']:run(args,name,240 if name=='candidate-core-compile' else 45,cpu,cwd=P['compile_cwd'])
 for name,args in P['links']:run(args,name+'-link',45,cpu,cwd=P['link_cwd'])
 S.update(candidate_core_sha256=sha(ROOT/'core-candidate.o'),candidate_type_probe_sha256=sha(ROOT/'type-probe'));save()
 env={k:v for k,v in os.environ.items() if not k.startswith(('STRATA_','ASTER_XQ_','DS41_','MIMO_','GGML_CUDA_'))}
 env.update(CUDA_VISIBLE_DEVICES='0',GGML_NO_BACKTRACE='1',OMP_NUM_THREADS='16',OPENBLAS_NUM_THREADS='1',ASTER_XQ_INDEXER_MASK_SKIP='0',ASTER_XQ_INDEXER_MXFP4_HEADER='0')
 for uid in ['0','1']:
  for guard,expected in [('0',3),('1',0)]:
   name='type-switch-uid'+uid+'-guard'+guard
   out,err=run([str(ROOT/'type-probe'),str(ROOT/(name+'.bin')),uid],name,15,dict(env,ASTER_XQ_GRAPH_SOURCE_TYPES=guard),expected)
   summary='RESULT runs12 values780 mismatches'+('390' if guard=='0' else '0')
   if summary not in out:raise RuntimeError('full isolated type-switch positive/negative control incomplete')
   S['type_switch_tests'].append(dict(nonzero_UID=uid=='1',guard=guard,values=780,mismatches=390 if guard=='0' else 0));save()
 # Independent numerical reference: qualified original CUDA core, no CUDA graphs.
 path=ROOT/'direct-oracle.scores.bin'
 out,err=run([str(ROOT/'direct-oracle'),str(path),'0'],'original-direct-oracle',90,dict(env,GGML_CUDA_DISABLE_GRAPHS='1'))
 if 'PASS actual CUDA indexer replay score cases: 270;' not in out:raise RuntimeError('complete direct numerical oracle required')
 reference=score(path,270);S['direct_oracle_sha256']=hashlib.sha256(reference).hexdigest();save()
 for name,binary,skip,mask,header in [('guarded-original','replay-original','0','0','0'),('candidate-off','replay-candidate','0','0','0'),
  ('premask-no-skip','replay-candidate','0','1','0'),('existing-mask-skip','replay-candidate','1','1','0'),('MXFP4-header-on','replay-candidate','1','1','1')]:
  dest=ROOT/(name+'.scores.bin')
  out,err=run([str(ROOT/binary),str(dest),mask],name,90,dict(env,ASTER_XQ_GRAPH_SOURCE_TYPES='1',ASTER_XQ_INDEXER_MASK_SKIP=skip,ASTER_XQ_INDEXER_MXFP4_HEADER=header))
  if 'PASS actual CUDA indexer replay score cases: 270;' not in out:raise RuntimeError('270 actual same-executable graph replays required')
  raw=score(dest,270);equal=raw==reference
  S['comparisons'].append(dict(profile=name,cases=270,score_values=416340,sha256=hashlib.sha256(raw).hexdigest(),byte_equal_to_original_direct_oracle=equal));save()
  if not equal:raise RuntimeError(name+' full-score byte identity differs from original direct oracle')
 out,err=run(['/usr/local/cuda/bin/compute-sanitizer','--tool','memcheck','--error-exitcode','99',P['MXFP4_probe']],'MXFP4-memcheck',210,env)
 if 'ERROR SUMMARY: 0 errors' not in out+err:raise RuntimeError('MXFP4 helper memcheck zero errors required')
 S.update(phase='passed',input_type_cache_defect_fixed_in_isolated_GPU_test=True,UID_fastpath_guard_tested=True,
  original_direct_reference_cases=270,guarded_replay_cases=1350,original90_gate_prior_cases=450,
  actual_CUDA_graph_launch_and_same_exec_changed_input_checked=True,full_score_byte_identity_passed=True,memcheck_zero_errors=True,
  model_quality_passed=False,formal_promoted=False,finished_unix=time.time())
except BaseException as e:S.update(phase='failed',error=type(e).__name__+': '+str(e),finished_unix=time.time());raise
finally:
 a=subprocess.run(['nvidia-smi','--query-compute-apps=pid','--format=csv,noheader,nounits'],capture_output=True,text=True,check=True,timeout=5).stdout.strip()
 S['GPU_idle_after']=not a;save()
