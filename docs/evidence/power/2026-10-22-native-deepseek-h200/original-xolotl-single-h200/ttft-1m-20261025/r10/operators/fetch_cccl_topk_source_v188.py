"""Fetch one official immutable release into this task's CPU source directory; no installation."""
import base64
import json
from xolotl_trial_common import BASE,jump,lease
ip=BASE/'xolotl-cccl-topk-v188-intent.json'
if ip.exists():raise RuntimeError('exact upstream source fetch already has a handle; inspect it')
root=lease()['remote']+'/cccl-topk-source-v188-cpu'
worker='''import hashlib,json,os,tarfile,time,urllib.request
from pathlib import Path,PurePosixPath
root=Path(__file__).resolve().parent;result=dict(unix=time.time(),phase='fetching',GPU_executed=False,system_installed=False)
def save():
 p=root/'result.tmp';p.write_text(json.dumps(result,indent=2)+'\\n');p.replace(root/'result.json')
def get_json(url):
 with urllib.request.urlopen(urllib.request.Request(url,headers={'User-Agent':'aster-h200-source-proof'}),timeout=25) as response:raw=response.read(1048577)
 if len(raw)>1048576:raise RuntimeError('bounded official release metadata exceeded')
 return json.loads(raw)
try:
 os.umask(0o077);save()
 if os.getuid()!=10250 or root.resolve()!=root or root.stat().st_uid!=10250:raise RuntimeError('exact CPU source root owner differs')
 url='https://api.github.com/repos/NVIDIA/cccl/git/ref/tags/v3.5.0';tag=get_json(url);obj=tag['object']
 if obj['type']=='tag':obj=get_json('https://api.github.com/repos/NVIDIA/cccl/git/tags/'+obj['sha'])['object']
 sha=obj['sha']
 if obj['type']!='commit' or len(sha)!=40 or any(c not in '0123456789abcdef' for c in sha) or not sha.startswith('28101b0'):raise RuntimeError('official immutable3.5 release commit differs')
 result.update(tag='v3.5.0',commit=sha,tag_metadata_url=url,archive_url='https://codeload.github.com/NVIDIA/cccl/tar.gz/'+sha);save()
 digest=hashlib.sha256();size=0;archive=root/'source.tar.gz'
 with urllib.request.urlopen(urllib.request.Request(result['archive_url'],headers={'User-Agent':'aster-h200-source-proof'}),timeout=40) as response,archive.open('xb') as output:
  while chunk:=response.read(1024**2):
   size+=len(chunk)
   if size>128*1024**2:raise RuntimeError('bounded official CCCL source archive exceeded')
   digest.update(chunk);output.write(chunk)
 result.update(phase='extracting_headers',archive_bytes=size,archive_sha256=digest.hexdigest());save()
 directory=root/'headers';directory.mkdir(mode=0o700);files=[];total=0;prefix='cccl-'+sha+'/'
 with tarfile.open(archive,'r:gz') as source:
  for member in source:
   if not member.name.startswith(prefix):raise RuntimeError('exact official archive root prefix differs')
   relative=member.name[len(prefix):];path=PurePosixPath(relative)
   selected=relative.startswith(('cub/cub/','thrust/thrust/','libcudacxx/include/')) or relative in ['LICENSE','LICENSE.TXT','LICENSE.BSD','LICENSE.APACHE']
   if not selected or member.isdir():continue
   if not member.isfile() or member.size>2*1024**2 or path.is_absolute() or '..' in path.parts or '\\\\' in relative:raise RuntimeError('bounded exact regular header archive member differs')
   total+=member.size
   if total>128*1024**2 or len(files)>=20000:raise RuntimeError('bounded extracted header set exceeded')
   data=source.extractfile(member).read(member.size+1)
   if len(data)!=member.size:raise RuntimeError('exact official header byte size differs')
   target=directory.joinpath(*path.parts);target.parent.mkdir(parents=True,exist_ok=True)
   with target.open('xb') as output:output.write(data)
   files.append(dict(file=relative,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
 needed=['cub/cub/device/device_topk.cuh','libcudacxx/include/cuda/execution','libcudacxx/include/cuda/std/execution']
 if any(not (directory/name).is_file() for name in needed):raise RuntimeError('official required Top-K/execution headers missing')
 (root/'header-manifest.json').write_text(json.dumps(dict(tag='v3.5.0',commit=sha,files=files),indent=2)+'\\n')
 result.update(phase='prepared',headers=str(directory),header_files=len(files),header_bytes=total,finished_unix=time.time());save()
except BaseException as error:result.update(phase='failed',error=type(error).__name__+': '+str(error)[:2000],finished_unix=time.time());save();raise
'''
compile(worker,'bounded-official-CCCL-fetch-v188','exec')
code='''import base64,json,os,subprocess,time
from pathlib import Path
root=Path(@ROOT@)
if root.exists() or os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250:raise RuntimeError('exclusive exact upstream CPU source stage differs')
os.umask(0o077);root.mkdir(mode=0o700)
with (root/'worker.py').open('xb') as output:output.write(base64.b64decode(@DATA@))
with (root/'worker.log').open('xb') as log:q=subprocess.Popen(['timeout','--signal=TERM','--kill-after=10','180','python3','-u',str(root/'worker.py')],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
handle=dict(pid=q.pid,start_ticks=int(Path('/proc/'+str(q.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),namespace='jump host',budget_seconds=180)
(root/'process.json').write_text(json.dumps(handle,indent=2)+'\\n');print(json.dumps(handle))
'''.replace('@ROOT@',repr(root)).replace('@DATA@',repr(base64.b64encode(worker.encode()).decode()))
r=dict(root=root,phase='launch_requested',GPU_executed=False)
with ip.open('x') as output:output.write(json.dumps(r,indent=2)+'\n')
r.update(process=jump(code),phase='launched');ip.write_text(json.dumps(r,indent=2)+'\n')
print(json.dumps(r))
