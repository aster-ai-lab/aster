from pathlib import Path
import hashlib,json,os,subprocess,time,zipfile
root=Path.cwd()
out=root/'.tmp/box-plugin-owner'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=json.loads((out/'source-before.json').read_text())
assert all(sha(root/p)==d for p,d in sources.items())
builder=root/'.tmp/plugins-box-maintained/aster-use-builder-current'
builder_digest=sha(builder)
assert builder_digest=='2d2784edcf1ca58c6dd755a67eceb971fd227722d2ae67b8f41b14f1d71e7d23'
argv=['python3.12',str(root/'plugins/recipes/aster-box/darwin-arm64.py'),str(root/'.tmp/plugin-registry/box-aix-candidate/aster-box-darwin-arm64.tar.gz'),'--aster-use-bin',str(builder),'--out',str(out/'qualified-final')]
with (out/'final-package.log').open('w',encoding='utf-8') as log:
 p=subprocess.run(argv,stdout=log,stderr=subprocess.STDOUT)
assert p.returncode==0
staged=out/'qualified-final/repository/plugins/operations/aster/aster-box'
aix=out/'qualified-final/aster-box-0.1.0-darwin-arm64.aix'
with zipfile.ZipFile(aix) as archive:
 paths={p.relative_to(staged).as_posix():p for p in staged.rglob('*') if p.is_file()}
 assert set(archive.namelist())==set(paths)
 assert all(archive.read(n)==p.read_bytes() for n,p in paths.items())
for p in (root/'plugins/plugins/operations/aster/aster-box').iterdir():
 assert p.read_bytes()==(staged/p.name).read_bytes()
audit={'aixSha256':sha(aix),'aixLength':aix.stat().st_size,'allStagedFilesMatchArchive':True,'allCanonicalFilesMatchStagedSource':True,'members':[{'path':n,'sha256':sha(p),'length':p.stat().st_size} for n,p in sorted(paths.items())]}
(out/'final-source-archive-audit.json').write_text(json.dumps(audit,indent=2)+'\n',encoding='utf-8')
inputs=out/'native-inputs-final.json'
inputs.write_text(json.dumps([{'packageId':'aster/aster-box','aix':str(aix),'version':'3.2.7','tool':'aster-box','versionArgs':['--version'],'requiredCommands':['aster-box'],'nativeTarget':'aarch64-apple-darwin'}],indent=2)+'\n',encoding='utf-8')
initial=json.loads((out/'native-initial-result.json').read_text())
runner=out/'studio-native-tests'
runner_digest=sha(runner)
assert runner_digest==initial['runner']['sha256Before']
argv=[str(runner),'capabilities::toolchain::tests::box_native::actual_box_aix_executes_linux_commands_before_package_retirement','--ignored','--exact','--nocapture']
env={**os.environ,'ASTER_TEST_DEPENDENCY_AIX_INPUTS':str(inputs),'ASTER_TEST_DEPENDENCY_AIX_EVIDENCE':str(out/'native-management-final.json'),'ASTER_TEST_BOX_OCI_FIXTURE':str(root/'.tmp/box-plugin-vm-probe/alpine-arm64.oci.tar')}
t=time.monotonic()
with (out/'native-final.log').open('w',encoding='utf-8') as log:
 p=subprocess.run(argv,env=env,stdout=log,stderr=subprocess.STDOUT)
report={'argv':argv,'exitCode':p.returncode,'seconds':time.monotonic()-t,'aixSha256':sha(aix),'aixLength':aix.stat().st_size,'sourcesUnchanged':all(sha(root/p)==d for p,d in sources.items()),'builderSha256':sha(builder),'runnerSha256':sha(runner),'binariesUnchanged':sha(builder)==builder_digest and sha(runner)==runner_digest,'published':False}
(out/'native-final-result.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
assert p.returncode==0 and report['sourcesUnchanged'] and report['binariesUnchanged']
assert 'test result: ok. 1 passed; 0 failed;' in (out/'native-final.log').read_text()
v=json.loads((out/'native-management-final.json').read_text())
assert v['results'][0]['aixArchiveSha256']=='sha256:'+report['aixSha256']
assert v['results'][0]['nativeConsumer']['nativeVmExecutionAccepted']
print('Final maintained Box AIX prepared, all source bytes matched, owner VM lifecycle passed',flush=True)
