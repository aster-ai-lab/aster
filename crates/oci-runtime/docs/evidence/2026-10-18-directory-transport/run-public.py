from pathlib import Path
import json,subprocess,tempfile,os,signal,time,uuid,hashlib
materials=Path(__file__).parent.resolve()
assert not subprocess.check_output(['git','status','--porcelain'],text=True).strip(), 'source must be clean'
revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
candidate=json.loads((materials/'candidate.json').read_text());binary=Path(candidate['root'])
for name,expected in candidate['sha256'].items():assert hashlib.sha256((binary/name).read_bytes()).hexdigest()==expected
root=Path(tempfile.mkdtemp(prefix='oh-',dir='/private/var/tmp'))
args=[str(binary/'aster-oci'),'macos-hvf-host-service-smoke','--shim',str(binary/'aster-oci-krun-shim'),'--system-image-manifest',str(materials/'clean-system/system-image.json'),'--bundle',str(materials.parent/'macos-vmm-confinement-20261018/fixture'),'--work-parent',str(root),'--iterations','25','--source-revision',revision]
meta={'root':str(root),'source_revision':revision,'candidate':candidate,'args':args}
(materials/'public-location.json').write_text(json.dumps(meta,indent=2)+'\n')
sentinel='ASTER_OCI_TEST_AMBIENT_SENTINEL='+uuid.uuid4().hex
environment=dict(os.environ);environment.update([sentinel.split('=',1)])
observed=set();leaked=False;deadline=time.monotonic()+900;timed_out=False
with (root/'stdout.json').open('wb') as output,(root/'stderr.log').open('wb') as error:
 proc=subprocess.Popen(args,stdout=output,stderr=error,env=environment,start_new_session=True)
 try:
  while True:
   try:code=proc.wait(timeout=1);break
   except subprocess.TimeoutExpired:pass
   if time.monotonic()>=deadline:
    timed_out=True;os.killpg(proc.pid,signal.SIGTERM)
    try:code=proc.wait(timeout=10)
    except subprocess.TimeoutExpired:os.killpg(proc.pid,signal.SIGKILL);code=proc.wait()
    break
   rows={}
   for line in subprocess.check_output(['ps','-axo','pid=,ppid=,command='],text=True,timeout=3).splitlines():
    fields=line.strip().split(None,2)
    if len(fields)==3:rows[int(fields[0])]=(int(fields[1]),fields[2])
   descendants={proc.pid};changed=True
   while changed:
    more={pid for pid,(parent,_) in rows.items() if parent in descendants};changed=not more.issubset(descendants);descendants.update(more)
   for pid in descendants-observed:
    command=rows.get(pid,(0,''))[1]
    if str(binary) in command and '__macos-agent-vm-worker' in command:
     probe=subprocess.run(['ps','eww','-p',str(pid),'-o','command='],capture_output=True,timeout=3)
     if probe.returncode==0 and b'__macos-agent-vm-worker' in probe.stdout:
      observed.add(pid);leaked|=sentinel.encode() in probe.stdout
 finally:
  if proc.poll() is None:
   os.killpg(proc.pid,signal.SIGTERM)
   try:proc.wait(timeout=10)
   except subprocess.TimeoutExpired:os.killpg(proc.pid,signal.SIGKILL);proc.wait()
meta.update({'exit_code':code,'timed_out':timed_out,'environment_probe':{'observed_worker_count':len(observed),'ambient_sentinel_leaked':leaked}})
(materials/'public-summary.json').write_text(json.dumps(meta,indent=2)+'\n');print(json.dumps({'root':str(root),'exit_code':code,'environment_probe':meta['environment_probe']}),flush=True)
raise SystemExit(code if code else (3 if leaked or not observed else 0))
