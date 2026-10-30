"""Guarded GPU6-only experiment rollout. Input is a nonsecret measurement recipe."""
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys
import time
import urllib.request

B=pathlib.Path('/data/deepseek-v41-xq-1017')
CFG=pathlib.Path('/etc/a3s/power/deepseek-gpu6.acl')
UNIT='a3s-power-deepseek-gpu6.service'
UP=pathlib.Path('/etc/systemd/system')/UNIT
C=json.loads(pathlib.Path(sys.argv[1]).read_text())
OUT=B/C['receipt']
PRESERVED=['a3s-gateway.service',*[f'prism-27b@{i}.service' for i in range(6)],*[f'a3s-power@{i}.service' for i in range(6)],'qwen3-asr.service','qwen3-asr-b.service']
proof={'at_epoch':time.time(),'profile':C['profile'],'activated':False,'success':False}

def sha(path):
    with pathlib.Path(path).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()

def props(unit):
    return dict(l.split('=',1) for l in subprocess.check_output(['systemctl','show',unit,'-p','MainPID','-p','ActiveState'],text=True).splitlines())

def get(url):
    with urllib.request.urlopen(url,timeout=5) as r:return json.load(r)

def save(phase):
    proof.update(phase=phase,updated_at_epoch=time.time())
    OUT.write_text(json.dumps(proof,indent=2));OUT.chmod(0o644)
    print(json.dumps({'phase':phase,'profile':C['profile']}),flush=True)

def atomic(source,target):
    temp=target.with_name('.'+target.name+'-dspark-'+str(os.getpid()))
    meta=source.stat();shutil.copy2(source,temp);os.chown(temp,meta.st_uid,meta.st_gid)
    with temp.open('rb') as f:os.fsync(f.fileno())
    os.replace(temp,target)

def reload_unit():
    subprocess.run(['systemctl','daemon-reload'],check=True,capture_output=True,timeout=30)

def restart():
    subprocess.run(['systemctl','restart',UNIT],check=True,capture_output=True,timeout=55)

def ready():
    deadline=time.monotonic()+1800
    while time.monotonic()<deadline:
        try:
            p=get('http://127.0.0.1:18206/health');n=get('http://127.0.0.1:18106/health')
            if p.get('status')=='ok' and p.get('loaded_models')==1 and n.get('loaded') is True and n.get('max_context')==1048576:return p,n
        except Exception:pass
        if props(UNIT)['ActiveState']=='failed':raise RuntimeError('GPU6 candidate failed during startup')
        time.sleep(1)
    raise RuntimeError('GPU6 startup exceeded its admitted 1800-second deadline')

old=None;old_unit=None;new_unit_sha=None
try:
    if os.geteuid()!=0:raise RuntimeError('Root required')
    if sha(CFG)!=C['old_cfg_sha'] or sha(UP)!=C['old_unit_sha'] or props(UNIT)!={'MainPID':str(C['old_pid']),'ActiveState':'active'}:raise RuntimeError('Active GPU6 identity changed')
    if sha('/proc/'+str(C['old_pid'])+'/exe')!=C['old_power_sha']:raise RuntimeError('Active Power executable changed')
    if sha(C['power'])!=C['power_sha'] or sha(C['bridge'])!=C['bridge_sha']:raise RuntimeError('Candidate artifact changed')
    staged=pathlib.Path(C['staged'])
    if staged.parent!=CFG.parent or not staged.name.startswith('.deepseek-gpu6-exact-resource-') or sha(staged)!=C['staged_sha']:raise RuntimeError('Staged native ACL changed')
    subprocess.run([C['power'],'serve','--config',str(staged),'--check-config'],check=True,capture_output=True,timeout=15)
    original_unit=UP.read_text();old_line='ExecStart='+C['old_power']+' serve --config '+str(CFG)
    if original_unit.splitlines().count(old_line)!=1:raise RuntimeError('Native service command changed')
    unit_text=original_unit.replace(old_line,'ExecStart='+C['power']+' serve --config '+str(CFG))
    stage_unit=UP.with_name('.a3s-power-dspark-'+str(os.getpid())+'.service')
    with stage_unit.open('x') as f:f.write(unit_text);f.flush();os.fsync(f.fileno())
    stage_unit.chmod(0o644);new_unit_sha=sha(stage_unit)
    subprocess.run(['systemd-analyze','verify',str(stage_unit)],check=True,capture_output=True,timeout=15)
    proof['preserved_services_before']={u:props(u) for u in PRESERVED}
    deadline=time.monotonic()+900;idle_since=None;save('waiting_for_foreground_idle')
    while time.monotonic()<deadline:
        admission=get('http://127.0.0.1:18206/health')['worker']['admission'];status=get('http://127.0.0.1:18106/status')
        if admission['active']==0 and admission['waiting']==0 and status.get('busy') is False and not status.get('queued',0):
            if idle_since is None:idle_since=time.monotonic()
            elif time.monotonic()-idle_since>=2:break
        else:idle_since=None
        time.sleep(.2)
    else:raise RuntimeError('No idle GPU6 cutover window')
    if sha(CFG)!=C['old_cfg_sha'] or sha(UP)!=C['old_unit_sha'] or props(UNIT)['MainPID']!=str(C['old_pid']):raise RuntimeError('GPU6 identity changed while draining')
    backup=CFG.parent/'backups';backup.mkdir(mode=0o700,exist_ok=True)
    old=backup/('before-dspark-'+C['old_cfg_sha'][:16]+'.acl')
    old_unit=backup/('before-dspark-'+C['old_unit_sha'][:16]+'.service')
    for src,dst in [(CFG,old),(UP,old_unit)]:
        if dst.exists():
            if sha(src)!=sha(dst):raise RuntimeError('Private backup collision')
        else:atomic(src,dst)
    proof['activated']=True
    atomic(staged,CFG);atomic(stage_unit,UP);reload_unit()
    proof.update(old_acl_sha256=C['old_cfg_sha'],new_acl_sha256=C['staged_sha'],new_unit_sha256=new_unit_sha,power_binary_sha256=C['power_sha'],bridge_sha256=C['bridge_sha'])
    save('loading_gpu6_candidate');restart();ready();save('validating_exact_arithmetic')
    body={'model':'deepseek-v41-flash-1017-xq-1m','messages':[{'role':'user','content':'Return a JSON object with one key answer, whose integer value is (19*23)-(17*11). No Markdown.'}],'max_tokens':512,'temperature':0,'seed':42,'stream':False}
    request=urllib.request.Request('http://127.0.0.1:18106/v1/chat/completions',data=json.dumps(body).encode(),headers={'Content-Type':'application/json'})
    with urllib.request.urlopen(request,timeout=180) as r:reply=json.load(r)
    (B/(C['receipt']+'.smoke.json')).write_text(json.dumps(reply,indent=2))
    if reply['choices'][0]['finish_reason']!='stop' or json.loads(reply['choices'][0]['message']['content'])!={'answer':250}:raise RuntimeError('Candidate arithmetic mismatch')
    engines=[]
    for row in subprocess.check_output(['nvidia-smi','--query-compute-apps=gpu_uuid,pid','--format=csv,noheader'],text=True).splitlines():
        gpu,pid=(v.strip() for v in row.split(','))
        if gpu!='GPU-ab93a347-c678-1d71-0e85-0f520463debe':continue
        argv=pathlib.Path('/proc/'+pid+'/cmdline').read_bytes().decode().rstrip('\0').split('\0')
        if '--serve' not in argv:continue
        for flag,value in [('--ctx','1048576'),('--threads','24'),('--arena-gib','144'),('--pcie','0.0000'),('--tier-batch-pcie',f"{C['tier_batch_pcie_bp']/10000:.4f}"),('--prefill-chunk',str(C['prefill_chunk'])),('--prompt-batch',str(C['prompt_batch'])),('--route-bias','0'),('--route-bias-file','0'),('--route-bias-remote','0')]:
            if argv.count(flag)!=1 or argv[argv.index(flag)+1]!=value:raise RuntimeError('Actual native argv differs from typed profile')
        if argv.count('--kv-fp4')!=1:raise RuntimeError('Packed KV flag missing')
        env_raw=pathlib.Path('/proc/'+pid+'/environ').read_bytes().split(b'\0')
        keys=['STRATA_SERVE_DSPARK','STRATA_DSPARK_DEPTH','STRATA_DSPARK_CONF','STRATA_SERVE_DSPARK_EVICT','STRATA_INDEXER_TILE']
        env={k:v for raw in env_raw if b'=' in raw for k,v in [raw.decode().split('=',1)] if k in keys}
        actual_tile=env.pop('STRATA_INDEXER_TILE',None)
        expected_tile=str(C['indexer_tile_elements']) if C.get('indexer_tile_elements') is not None else None
        if actual_tile!=expected_tile:raise RuntimeError('Actual indexer tile differs from typed ACL')
        if env!=C['dspark_env']:raise RuntimeError('Actual draft environment differs from typed ACL')
        engines.append({'pid':int(pid),'argv':argv,'dspark_env':env,'indexer_tile_elements':actual_tile})
    if len(engines)!=1:raise RuntimeError('Expected one owned GPU6 engine')
    proof['preserved_services_after']={u:props(u) for u in PRESERVED}
    if proof['preserved_services_after']!=proof['preserved_services_before']:raise RuntimeError('Another model service identity changed')
    if sha(CFG)!=C['staged_sha'] or sha(UP)!=new_unit_sha:raise RuntimeError('Concurrent native deployment')
    proof.update(success=True,new_pid=int(props(UNIT)['MainPID']),engine=engines[0],other_service_pids_unchanged=True,smoke_timings=reply.get('timings'))
    save('ready_for_controlled_measurement')
except BaseException as error:
    proof.update(error_type=type(error).__name__,error=str(error))
    if proof['activated'] and old is not None and old_unit is not None:
        try:
            if sha(CFG) not in [C['staged_sha'],C['old_cfg_sha']] or sha(UP) not in [new_unit_sha,C['old_unit_sha']]:raise RuntimeError('Concurrent change prevents fallback')
            atomic(old,CFG);atomic(old_unit,UP);reload_unit();restart();ready();proof['previous_profile_restored']=True
        except BaseException as fallback:proof['fallback_error']=type(fallback).__name__+': '+str(fallback)
    save('failed');raise
