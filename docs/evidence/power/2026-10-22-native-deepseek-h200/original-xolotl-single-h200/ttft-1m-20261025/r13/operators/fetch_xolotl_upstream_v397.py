"""Fetch one exact upstream source archive into the existing workspace, no new Git repository."""
import hashlib,json,stat,time,urllib.request,zipfile
from pathlib import Path,PurePosixPath
B=Path(__file__).resolve().parent;head='0b9a0fc86dc55388c8c737bdc5fbccccf7273927';root=B/('xolotl-upstream-'+head[:12]);archive=B/('xolotl-upstream-'+head[:12]+'.zip')
if root.exists() or archive.exists():raise RuntimeError('exact upstream source fetch already has artifacts; inspect them')
url='https://codeload.github.com/Indras-Mirror/Xolotl/zip/'+head;count=0;h=hashlib.sha256()
req=urllib.request.Request(url,headers={'User-Agent':'aster-deployment-source-review'})
with urllib.request.urlopen(req,timeout=40) as r,archive.open('xb') as f:
 while chunk:=r.read(1024**2):
  count+=len(chunk)
  if count>256*1024**2:raise RuntimeError('bounded upstream archive exceeded')
  h.update(chunk);f.write(chunk)
rows=[];total=0
with zipfile.ZipFile(archive) as z:
 members=z.infolist()
 if len(members)>20000:raise RuntimeError('bounded upstream source inventory exceeded')
 prefix='Xolotl-'+head
 for m in members:
  p=PurePosixPath(m.filename);total+=m.file_size
  if p.is_absolute() or '..' in p.parts or not p.parts or p.parts[0]!=prefix or stat.S_ISLNK(m.external_attr>>16) or m.file_size>32*1024**2 or total>512*1024**2:raise RuntimeError('unsafe or oversized upstream source entry')
 root.mkdir()
 for m in members:
  p=PurePosixPath(m.filename)
  if len(p.parts)==1:continue
  dest=root.joinpath(*p.parts[1:])
  if m.is_dir():dest.mkdir(parents=True,exist_ok=True);continue
  raw=z.read(m);dest.parent.mkdir(parents=True,exist_ok=True)
  with dest.open('xb') as f:f.write(raw)
  rows.append(dict(file=dest.relative_to(root).as_posix(),bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()))
proof=dict(unix=time.time(),phase='fetched_exact',source='https://github.com/Indras-Mirror/Xolotl',commit=head,archive_url=url,archive_bytes=count,archive_sha256=h.hexdigest(),root=str(root),files=rows,active_build_changed=False,model_requests_sent=False,new_git_repository_created=False)
with (B/'xolotl-upstream-fetch-v397.json').open('x') as f:f.write(json.dumps(proof,indent=2)+'\n')
print(json.dumps(dict(phase=proof['phase'],commit=head,archive_bytes=count,source_files=len(rows),active_build_changed=False)))
