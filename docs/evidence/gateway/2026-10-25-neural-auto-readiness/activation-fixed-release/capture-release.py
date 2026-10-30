import hashlib,json,os,pathlib,shutil,subprocess,time
out=pathlib.Path(__file__).resolve().parent
cfg=json.loads((out/'capture-release-config.json').read_text())
env=os.environ.copy()
env['PATH']=str(pathlib.Path.home()/'.cargo/bin')+':'+env.get('PATH','')
env.update(CARGO_TARGET_DIR='/code/aster/target',CARGO_BUILD_JOBS='2',CARGO_NET_OFFLINE='false',CARGO_TERM_COLOR='never')
root=pathlib.Path(cfg['root'])
with (out/'gateway-release-artifacts.jsonl').open('w') as stdout,(out/'gateway-release-artifacts.stderr.log').open('w') as stderr:
 result=subprocess.run(cfg['command']+['--message-format=json-render-diagnostics'],cwd=root,env=env,stdout=stdout,stderr=stderr)
assert result.returncode==0
artifacts=[json.loads(line) for line in (out/'gateway-release-artifacts.jsonl').read_text().splitlines() if line.startswith('{')]
candidates=[a for a in artifacts if a.get('reason')=='compiler-artifact' and a.get('target',{}).get('name')=='a3s-gateway' and a.get('executable')]
assert len(candidates)==1
a=candidates[0]
assert a['manifest_path']==str(root/'apps/gateway/Cargo.toml')
exe=pathlib.Path(a['executable'])
inode=exe.stat().st_ino
deps=[p for p in (exe.parent/'deps').glob('a3s_gateway-*') if p.is_file() and p.stat().st_ino==inode]
assert len(deps)==1
source=deps[0]
before=source.stat()
stage=pathlib.Path(cfg['stage'])
stage.mkdir(exist_ok=True)
target=stage/'a3s-gateway'
shutil.copy2(source,target)
after=source.stat()
assert (before.st_ino,before.st_size,before.st_mtime_ns)==(after.st_ino,after.st_size,after.st_mtime_ns)
def sha(p):
 with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
digest=sha(target)
assert sha(source)==digest
receipt={'exit_code':0,'source_root':str(root/'apps/gateway'),'command':cfg['command']+['--message-format=json-render-diagnostics'],'compiler_artifact':a,'unique_dependency_executable':str(source),'captured_executable':str(target),'sha256':digest,'size_bytes':target.stat().st_size,'capture_stable':True,'at_epoch':time.time()}
(out/'captured-gateway-release.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({k:receipt[k] for k in ['exit_code','sha256','size_bytes','capture_stable']}),flush=True)
