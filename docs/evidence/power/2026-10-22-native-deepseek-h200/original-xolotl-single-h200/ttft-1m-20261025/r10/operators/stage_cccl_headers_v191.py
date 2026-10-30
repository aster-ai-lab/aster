"""Transfer the pinned official header archive and extract only bounded regular include/license files."""
import base64
import hashlib
import json
from pathlib import Path
import subprocess
from xolotl_trial_common import BASE,SSH,jump,lease

ip=BASE/'xolotl-cccl-headers-v191-intent.json'
if ip.exists():raise RuntimeError('exact CCCL header stage already has a handle; inspect it')
source=json.loads((BASE/'cccl-topk-local-v189r1/result.json').read_text())
archive=Path(source['archive_path'])
if source['phase']!='downloaded' or source['commit']!='28101b07413743e1dcca8ca8af1cfc63e8cff2d1' or archive.stat().st_size!=source['archive_bytes'] or hashlib.sha256(archive.read_bytes()).hexdigest()!=source['archive_sha256']:
    raise RuntimeError('exact official local archive lineage differs')
root=lease()['remote']+'/cccl-topk-headers-v191-cpu'
record=dict(root=root,phase='prepared',source=source,GPU_executed=False,system_installed=False)
with ip.open('x') as output:output.write(json.dumps(record,indent=2)+'\n')
code='''import json,os
from pathlib import Path
root=Path(@ROOT@)
if root.exists() or os.getuid()!=10250 or root.parent.resolve()!=root.parent or root.parent.stat().st_uid!=10250:raise RuntimeError('exclusive exact CCCL header root differs')
os.umask(0o077);root.mkdir(mode=0o700);print(json.dumps(dict(created=True)))
'''.replace('@ROOT@',repr(root))
record['stage']=jump(code);record['phase']='transferring';ip.write_text(json.dumps(record,indent=2)+'\n')
command=['scp','-C',*SSH[2:-1],str(archive),SSH[-1]+':'+root+'/source.tar.gz']
q=subprocess.run(command,capture_output=True,text=True,timeout=90)
record['transfer_returncode']=q.returncode;record['transfer_stderr']=q.stderr[-3000:];ip.write_text(json.dumps(record,indent=2)+'\n')
if q.returncode:raise RuntimeError('exact official archive transfer failed; inspect existing root')
worker='''import hashlib,json,os,tarfile,time
from pathlib import Path,PurePosixPath
root=Path(__file__).resolve().parent;plan=json.loads((root/'plan.json').read_text());source=plan['source'];result=dict(unix=time.time(),phase='extracting',GPU_executed=False,system_installed=False)
def save():
 p=root/'result.tmp';p.write_text(json.dumps(result,indent=2)+'\\n');p.replace(root/'result.json')
try:
 save();archive=root/'source.tar.gz'
 if root.resolve()!=root or root.stat().st_uid!=10250 or archive.is_symlink() or archive.stat().st_uid!=10250 or archive.stat().st_size!=source['archive_bytes'] or hashlib.sha256(archive.read_bytes()).hexdigest()!=source['archive_sha256']:raise RuntimeError('exact transferred source archive differs')
 directory=root/'headers';directory.mkdir(mode=0o700);files=[];total=0;prefix='cccl-'+source['commit']+'/'
 with tarfile.open(archive,'r:gz') as stream:
  for member in stream:
   if member.isdir() and member.name.rstrip('/')==prefix.rstrip('/'):continue
   if not member.name.startswith(prefix):raise RuntimeError('exact official archive member root differs')
   relative=member.name[len(prefix):];path=PurePosixPath(relative)
   selected=relative.startswith(('cub/cub/','thrust/thrust/','libcudacxx/include/')) or relative in ['LICENSE','LICENSE.TXT','LICENSE.BSD','LICENSE.APACHE']
   if not selected or member.isdir():continue
   if not member.isfile() or member.size>2*1024**2 or path.is_absolute() or '..' in path.parts or '\\\\' in relative:raise RuntimeError('bounded regular official header member differs')
   total+=member.size
   if total>128*1024**2 or len(files)>=20000:raise RuntimeError('bounded extracted header set exceeded')
   data=stream.extractfile(member).read(member.size+1)
   if len(data)!=member.size:raise RuntimeError('exact declared header size differs')
   target=directory.joinpath(*path.parts);target.parent.mkdir(parents=True,exist_ok=True)
   with target.open('xb') as output:output.write(data)
   files.append(dict(file=relative,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
 needed=['cub/cub/device/device_topk.cuh','libcudacxx/include/cuda/execution','libcudacxx/include/cuda/std/execution']
 if any(not (directory/name).is_file() for name in needed):raise RuntimeError('official Top-K/execution headers missing')
 (root/'header-manifest.json').write_text(json.dumps(dict(tag='v3.5.0',commit=source['commit'],files=files),indent=2)+'\\n')
 result.update(phase='prepared',headers=str(directory),header_files=len(files),header_bytes=total,commit=source['commit'],archive_sha256=source['archive_sha256'],finished_unix=time.time());save()
except BaseException as error:result.update(phase='failed',error=type(error).__name__+': '+str(error)[:2000],finished_unix=time.time());save();raise
'''
compile(worker,'bounded-exact-official-header-extraction-v191','exec')
code='''import base64,json,os,subprocess,time
from pathlib import Path
r=@R@;root=Path(r['root'])
if root.resolve()!=root or root.stat().st_uid!=10250 or (root/'worker.py').exists():raise RuntimeError('exact transferred header activation differs')
os.umask(0o077)
with (root/'worker.py').open('xb') as output:output.write(base64.b64decode(@WORKER@))
with (root/'plan.json').open('x') as output:output.write(json.dumps(r,indent=2)+'\\n')
with (root/'worker.log').open('xb') as log:q=subprocess.Popen(['timeout','--signal=TERM','--kill-after=10','180','python3','-u',str(root/'worker.py')],cwd=root,env=dict(os.environ,CUDA_VISIBLE_DEVICES=''),stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True)
handle=dict(pid=q.pid,start_ticks=int(Path('/proc/'+str(q.pid)+'/stat').read_text().split(') ',1)[1].split()[19]),uid=os.getuid(),unix=time.time(),namespace='jump host',budget_seconds=180)
(root/'process.json').write_text(json.dumps(handle,indent=2)+'\\n');print(json.dumps(handle))
'''.replace('@R@',repr(record)).replace('@WORKER@',repr(base64.b64encode(worker.encode()).decode()))
record['phase']='launch_requested';ip.write_text(json.dumps(record,indent=2)+'\n')
record.update(process=jump(code),phase='launched');ip.write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(dict(phase=record['phase'],process=record['process'],archive_bytes=source['archive_bytes'],GPU_executed=False)))
