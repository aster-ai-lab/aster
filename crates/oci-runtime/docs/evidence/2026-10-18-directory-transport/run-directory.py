from pathlib import Path
import json, subprocess, tempfile, hashlib, signal, os, time
q=Path(__file__).parent.resolve()
assert not subprocess.check_output(['git','status','--porcelain'],text=True).strip()
candidate=json.loads((q/'candidate.json').read_text()); binary=Path(candidate['root'])
revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
assert revision==candidate['source_revision']
for name,digest in candidate['sha256'].items(): assert hashlib.sha256((binary/name).read_bytes()).hexdigest()==digest
root=Path(tempfile.mkdtemp(prefix='wd-',dir='/private/var/tmp'))
args=[str(binary/'qualify_hvf_workspace'),str(binary/'aster-oci'),str(binary/'aster-oci-krun-shim'),str(q/'clean-system/system-image.json'),str(q.parent/'macos-vmm-confinement-20261018/fixture'),str(root)]
meta={'root':str(root),'candidate':candidate,'source_revision':revision,'source_dirty':False,'args':args}
(q/'directory-location.json').write_text(json.dumps(meta,indent=2)+'\n')
started=time.monotonic(); timed_out=False
with (root/'stdout.json').open('wb') as out,(root/'stderr.log').open('wb') as err:
 proc=subprocess.Popen(args,stdout=out,stderr=err,start_new_session=True)
 try:
  try: code=proc.wait(timeout=210)
  except subprocess.TimeoutExpired: timed_out=True;code=124
 finally:
  if proc.poll() is None:
   os.killpg(proc.pid,signal.SIGTERM)
   try: proc.wait(timeout=10)
   except subprocess.TimeoutExpired:os.killpg(proc.pid,signal.SIGKILL);proc.wait()
meta.update({'exit_code':code,'timed_out':timed_out,'elapsed_seconds':time.monotonic()-started})
(q/'directory-summary.json').write_text(json.dumps(meta,indent=2)+'\n')
print(json.dumps(meta,indent=2))
raise SystemExit(code)
