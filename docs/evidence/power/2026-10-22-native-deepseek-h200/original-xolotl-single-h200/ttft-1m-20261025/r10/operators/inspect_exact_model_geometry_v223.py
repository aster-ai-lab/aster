"""Read declared model Top-K geometry with the actual original metadata API, CPU only."""
import base64
import hashlib
import json
from xolotl_trial_common import BASE,jump,lease
path=BASE/'xolotl-exact-topk-geometry-v223.json'
if path.exists():raise RuntimeError('exact model geometry already observed')
plan=json.loads((BASE/'xolotl-native-serve-profile-v168-plan.json').read_text());ncu=json.loads((BASE/'xolotl-ncu-actual1m-v172r2-plan.json').read_text())
source='''#include "ds41_geometry.hpp"
#include <cstdio>
int main(int argc,char** argv){
 if(argc!=2)return 2;
 auto model=strata::GgufModel::open(argv[1]);strata::v41::Ds41Geometry g;
 auto error=strata::v41::ds41_read_geometry(model,g);
 if(!error.empty()){std::fprintf(stderr,"%s\\n",error.c_str());return 3;}
 std::printf("{\\"phase\\":\\"passed\\",\\"index_topk\\":%lld,\\"index_heads\\":%lld,\\"index_dim\\":%lld,\\"candidate_blocks\\":%lld,\\"candidate_block_size\\":%lld,\\"context_length\\":%lld,\\"model_layers\\":%lld,\\"index_source_layers\\":[",(long long)g.idx_topk,(long long)g.idx_heads,(long long)g.idx_dim,(long long)g.cand_blocks,(long long)g.cand_bsize,(long long)g.ctx_len,(long long)g.n_layer);
 for(size_t i=0;i<g.index_sources.size();++i)std::printf("%s%lld",i?",":"",(long long)g.index_sources[i]);
 std::puts("]}");
}
'''
record=dict(root=lease()['remote']+'/geometry-topk-v223-cpu',build=plan['link_cwd'],model=ncu['model'],source_sha256=hashlib.sha256(source.encode()).hexdigest(),GPU_work_started=False)
code='''import base64,hashlib,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root']);raw=base64.b64decode(@DATA@)
if root.exists() or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250 or os.getuid()!=10250 or hashlib.sha256(raw).hexdigest()!=r['source_sha256']:raise RuntimeError('exclusive exact metadata probe stage differs')
os.umask(0o077);root.mkdir(mode=0o700);(root/'geometry.cpp').write_bytes(raw);build=Path(r['build']);source=build.parent/'source'
env={k:os.environ[k] for k in ['PATH','LD_LIBRARY_PATH'] if k in os.environ};env.update(CUDA_VISIBLE_DEVICES='',OMP_NUM_THREADS='1')
command=['nice','-n','19','/usr/bin/c++','-O2','-std=c++17','-I'+str(source/'include'),'-I'+str(source/'tools/v41/engine'),str(root/'geometry.cpp'),str(build/'libstrata_core.a'),'-lpthread','-ldl','-o',str(root/'geometry')]
with (root/'compile.log').open('xb') as output:q=subprocess.run(command,cwd=root,env=env,stdout=output,stderr=output,timeout=30)
if q.returncode:raise RuntimeError('exact metadata API probe compilation failed')
q=subprocess.run([str(root/'geometry'),r['model']],cwd=root,env=env,capture_output=True,text=True,check=True,timeout=15)
if len(q.stdout)>65536 or q.stderr:raise RuntimeError('bounded exact model geometry output differs')
result=dict(unix=time.time(),geometry=json.loads(q.stdout),GPU_executed=False,model_forward_executed=False,source_sha256=r['source_sha256']);(root/'result.json').write_text(json.dumps(result,indent=2)+'\\n');print(json.dumps(result))
'''.replace('@R@',repr(record)).replace('@DATA@',repr(base64.b64encode(source.encode()).decode()))
record['result']=jump(code)
with path.open('x') as out:out.write(json.dumps(record,indent=2)+'\n')
print(json.dumps(record['result'],indent=2))
