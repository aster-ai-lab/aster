import hashlib,json,os,pathlib,subprocess,time,urllib.request
out=pathlib.Path(__file__).parent
cfg=json.loads((out/'deploy-gateway-config.json').read_text())
def sha(p):
 with pathlib.Path(p).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def cmd(args,check=True):
 r=subprocess.run(args,capture_output=True,text=True,timeout=75)
 if check and r.returncode:raise RuntimeError("command refused: "+' '.join(args[:3])+"; "+r.stderr[-500:])
 return r
def prop(unit,key):return cmd(['systemctl','show',unit,'-p',key,'--value']).stdout.strip()
def pids():return {u:prop(u,'MainPID') for u in cfg['preserved_pids']}
def persist():
 (out/'deployment-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
receipt={'started_at_epoch':time.time(),'success':False,'gateway_binary_sha256':cfg['new_sha256'],'preserved_pids_before':pids(),'phase':'preflight'}
assert receipt['preserved_pids_before']==cfg['preserved_pids']
assert prop('a3s-gateway.service','MainPID')==cfg['old_pid']
assert sha('/proc/'+cfg['old_pid']+'/exe')==cfg['old_sha256']
assert sha(cfg['base_path'])==cfg['base_before_sha256']
assert sha(cfg['selected_base_path'])==cfg['base_selected_sha256']
assert sha(cfg['binary'])==cfg['new_sha256']
assert sha(cfg['old_release'])==cfg['old_sha256']
timer_was_active=prop('a3s-web-refresh.timer','ActiveState')=='active'
base=pathlib.Path(cfg['base_path']);old_base=base.read_bytes();base_stat=base.stat()
live=pathlib.Path('/opt/a3s/bin/a3s-gateway-unified')
switched=False
def replace_binary(target):
 tmp=live.with_name('.a3s-gateway-neural-auto-next')
 if tmp.is_symlink():tmp.unlink()
 assert not tmp.exists()
 tmp.symlink_to(target)
 os.replace(tmp,live)
def replace_base(data):
 tmp=base.with_name(base.name+'.neural-auto-next')
 with tmp.open('wb') as f:f.write(data);f.flush();os.fsync(f.fileno())
 os.chmod(tmp,base_stat.st_mode & 0o777);os.chown(tmp,base_stat.st_uid,base_stat.st_gid)
 os.replace(tmp,base)
try:
 cmd(['systemctl','stop','a3s-web-refresh.timer'])
 assert prop('a3s-web-refresh.service','ActiveState') not in ['active','activating']
 idle=0;deadline=time.monotonic()+90
 while time.monotonic()<deadline:
  quiet=True
  for port in range(18200,18207):
   with urllib.request.urlopen('http://127.0.0.1:'+str(port)+'/health',timeout=4) as r:health=json.load(r)
   admission=health.get('worker',{}).get('admission',{})
   quiet=quiet and admission.get('active')==0 and admission.get('waiting')==0
  with urllib.request.urlopen('http://127.0.0.1:18106/status',timeout=4) as r:native=json.load(r)
  quiet=quiet and not native.get('busy',True) and native.get('queued',1)==0
  idle=idle+1 if quiet else 0
  if idle>=2:break
  time.sleep(1)
 assert idle>=2,"model request queues did not become idle"
 assert pids()==cfg['preserved_pids']
 assert prop('a3s-gateway.service','MainPID')==cfg['old_pid']
 assert sha(cfg['base_path'])==cfg['base_before_sha256']
 release=pathlib.Path(cfg['release']);release.parent.mkdir(parents=True,exist_ok=True)
 cmd(['install','-m','0755',cfg['binary'],str(release)])
 assert sha(release)==cfg['new_sha256']
 replace_base(pathlib.Path(cfg['selected_base_path']).read_bytes())
 replace_binary(str(release));switched=True
 receipt['phase']='gateway-restarting';persist()
 cmd(['systemctl','restart','a3s-gateway.service'])
 assert prop('a3s-gateway.service','ActiveState')=='active'
 receipt['new_gateway_pid']=prop('a3s-gateway.service','MainPID')
 assert receipt['new_gateway_pid']!=cfg['old_pid']
 assert sha('/proc/'+receipt['new_gateway_pid']+'/exe')==cfg['new_sha256']
 receipt['phase']='official-managed-publish';persist()
 refresh=cmd(['systemctl','start','a3s-web-refresh.service'])
 receipt['official_refresh_exit']=refresh.returncode
 journal=json.loads(pathlib.Path(cfg['journal']).read_text())
 snap=journal['snapshot']
 receipt['snapshot']={k:snap.get(k) for k in ['gateway_id','revision','snapshot_digest','issued_at','expires_at']}
 receipt['preserved_pids_after']=pids()
 assert receipt['preserved_pids_after']==cfg['preserved_pids']
 receipt['phase']='completed';receipt['success']=True
except Exception as error:
 receipt['error_type']=type(error).__name__;receipt['error']=str(error)
 if switched:
  receipt['phase']='rolling-back';persist()
  replace_base(old_base);replace_binary(cfg['old_release'])
  cmd(['systemctl','restart','a3s-gateway.service'],False)
  rollback=cmd(['systemctl','start','a3s-web-refresh.service'],False)
  receipt['rollback_refresh_exit']=rollback.returncode
 receipt['phase']='failed'
finally:
 if timer_was_active:cmd(['systemctl','start','a3s-web-refresh.timer'],False)
 receipt['timer_active_after']=prop('a3s-web-refresh.timer','ActiveState')
 receipt['finished_at_epoch']=time.time();persist()
print(json.dumps({k:receipt.get(k) for k in ['success','phase','new_gateway_pid','gateway_binary_sha256','snapshot','timer_active_after','error']}),flush=True)
if not receipt['success']:raise SystemExit(1)
