"""Fetch official release through Windows after the retained jump-host network timeout."""
import hashlib
import json
from pathlib import Path
import time
import urllib.request
from xolotl_trial_common import BASE

directory=BASE/'cccl-topk-local-v189r1'
if directory.exists():raise RuntimeError('exact local official source fetch already exists; inspect it')
directory.mkdir();state=dict(unix=time.time(),phase='fetching',GPU_executed=False,system_installed=False,
    retained_remote_failure='xolotl-cccl-topk-v188-latest.json')
def save():(directory/'result.json').write_text(json.dumps(state,indent=2)+'\n')
def get_json(url):
    with urllib.request.urlopen(urllib.request.Request(url,headers={'User-Agent':'aster-h200-source-proof'}),timeout=20) as response:raw=response.read(1048577)
    if len(raw)>1048576:raise RuntimeError('bounded official metadata exceeded')
    return json.loads(raw)
try:
    save();url='https://api.github.com/repos/NVIDIA/cccl/git/ref/tags/v3.5.0';tag=get_json(url);obj=tag['object']
    for depth in range(4):
        if obj['type']!='tag':break
        obj=get_json('https://api.github.com/repos/NVIDIA/cccl/git/tags/'+obj['sha'])['object']
    else:raise RuntimeError('bounded annotated-tag chain exceeded')
    sha=obj['sha']
    if obj['type']!='commit' or len(sha)!=40 or any(c not in '0123456789abcdef' for c in sha) or not sha.startswith('28101b0'):raise RuntimeError('official immutable3.5 release commit differs')
    state.update(tag='v3.5.0',commit=sha,tag_metadata_url=url,archive_url='https://codeload.github.com/NVIDIA/cccl/tar.gz/'+sha);save()
    digest=hashlib.sha256();size=0;archive=directory/'source.tar.gz'
    with urllib.request.urlopen(urllib.request.Request(state['archive_url'],headers={'User-Agent':'aster-h200-source-proof'}),timeout=40) as response,archive.open('xb') as output:
        while chunk:=response.read(1024**2):
            size+=len(chunk)
            if size>128*1024**2:raise RuntimeError('bounded official source archive exceeded')
            digest.update(chunk);output.write(chunk)
    state.update(phase='downloaded',archive_bytes=size,archive_sha256=digest.hexdigest(),archive_path=str(archive),finished_unix=time.time());save()
    print(json.dumps(state))
except BaseException as error:
    state.update(phase='failed',error=type(error).__name__+': '+str(error)[:2000],finished_unix=time.time());save();raise
