from pathlib import Path
import json,subprocess,tempfile,time,signal,os,hashlib
q=Path(__file__).parent.resolve();candidate=json.loads((q/'candidate.json').read_text());binary=Path(candidate['root']);old=q.parent/'hvf-directory-transport-20261018';previous=json.loads((old/'candidate.json').read_text());oldbinary=Path(previous['root'])
assert not candidate['source_dirty']
assert not subprocess.check_output(['git','status','--porcelain'],text=True).strip()
assert subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()==candidate['source_revision']
for info in [candidate,previous]:
 for name,digest in info['sha256'].items():assert hashlib.sha256((Path(info['root'])/name).read_bytes()).hexdigest()==digest
results=[]
for name,shim,image in [('old-shim',oldbinary/'aster-oci-krun-shim',q/'clean-system/system-image.json'),('old-guest',binary/'aster-oci-krun-shim',old/'clean-system/system-image.json')]:
 root=Path(tempfile.mkdtemp(prefix='vn-',dir='/private/var/tmp'))
 args=[str(binary/'qualify_hvf_workspace'),str(binary/'aster-oci'),str(shim),str(image),str(q.parent/'macos-vmm-confinement-20261018/fixture'),str(root)]
 control=Path(tempfile.mkdtemp(prefix='vc-',dir='/private/var/tmp'))
 control_args=[str(oldbinary/'qualify_hvf_workspace'),str(binary/'aster-oci'),str(shim),str(image),str(q.parent/'macos-vmm-confinement-20261018/fixture'),str(control)]
 with (control/'stdout.json').open('wb') as out,(control/'stderr.log').open('wb') as err:
  child=subprocess.Popen(control_args,stdout=out,stderr=err,start_new_session=True)
  try:control_code=child.wait(timeout=210)
  finally:
   if child.poll() is None:
    os.killpg(child.pid,signal.SIGTERM)
    try:child.wait(timeout=10)
    except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()
 assert control_code==0,(name,'legacy positive control failed',str(control))
 started=time.monotonic();timed_out=False
 with (root/'stdout.json').open('wb') as out,(root/'stderr.log').open('wb') as err:
  proc=subprocess.Popen(args,stdout=out,stderr=err,start_new_session=True)
  try:
   try:code=proc.wait(timeout=210)
   except subprocess.TimeoutExpired:code=124;timed_out=True
  finally:
   if proc.poll() is None:
    os.killpg(proc.pid,signal.SIGTERM)
    try:proc.wait(timeout=10)
    except subprocess.TimeoutExpired:os.killpg(proc.pid,signal.SIGKILL);proc.wait()
 writable=root/'读写工作区';assert code!=0 and not timed_out,(name,code)
 assert not (writable/'中文.txt').exists() and not (writable/'.aster/loops/output').exists(),name
 assert (writable/'.env').read_bytes()==b'host-credential'
 logs=[]
 for path in root.rglob('*'):
  if path.is_file() and path.suffix=='.log' and path.stat().st_size<1048576:logs.append((str(path.relative_to(root)),path.read_text(errors='replace')))
 combined='\n'.join(text for _,text in logs)
 if name=='old-shim':
  assert '\\"vm_entered\\": false' in combined or '\"vm_entered\": false' in combined,(name,combined[-4000:])
 else:
  assert 'connecting the authenticated agent bridge' in combined,(name,combined[-4000:])
 results.append({'case':name,'root':str(root),'args':args,'exit_code':code,'elapsed_seconds':time.monotonic()-started,'payload_not_started':True,'credential_preserved':True,'legacy_control_root':str(control),'legacy_control_args':control_args,'legacy_control_exit_code':control_code,'required_policy_rejected':True,'logs':logs})
 (q/'negative-results.json').write_text(json.dumps({'candidate':candidate,'previous_candidate':previous,'cases':results},ensure_ascii=False,indent=2)+'\n')
 print(json.dumps({'case':name,'exit_code':code,'root':str(root)}),flush=True)
