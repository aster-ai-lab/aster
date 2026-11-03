from pathlib import Path
import hashlib,json,os,shutil,subprocess,time
root=Path.cwd()
out=root/'.tmp/box-plugin-owner'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sources=json.loads((out/'source-before.json').read_text())
assert all(sha(root/p)==d for p,d in sources.items())
text=(out/'focused-tests.log').read_text()
assert 'test result: ok. 6 passed; 0 failed; 4 ignored;' in text
lines=[line for line in text.splitlines() if 'Running unittests ' in line]
assert len(lines)==1,lines
relative=lines[0].rsplit('(',1)[1].removesuffix(')')
built=root/relative
runner=out/'studio-native-tests'
assert not runner.exists()
shutil.copy2(built,runner)
before=sha(runner)
inputs=root/'.tmp/plugins-box-maintained/native-inputs.json'
fixture=root/'.tmp/box-plugin-vm-probe/alpine-arm64.oci.tar'
argv=[str(runner),'capabilities::toolchain::tests::box_native::actual_box_aix_executes_linux_commands_before_package_retirement','--ignored','--exact','--nocapture']
env={**os.environ,'ASTER_TEST_DEPENDENCY_AIX_INPUTS':str(inputs),'ASTER_TEST_DEPENDENCY_AIX_EVIDENCE':str(out/'native-management-initial.json'),'ASTER_TEST_BOX_OCI_FIXTURE':str(fixture)}
start=time.monotonic()
with (out/'native-initial.log').open('w',encoding='utf-8') as log:
 p=subprocess.run(argv,env=env,stdout=log,stderr=subprocess.STDOUT)
report={'argv':argv,'exitCode':p.returncode,'seconds':time.monotonic()-start,'sourceSha256':sources,'sourcesUnchanged':all(sha(root/p)==d for p,d in sources.items()),'runner':{'source':str(built),'copy':str(runner),'length':runner.stat().st_size,'sha256Before':before,'sha256After':sha(runner)},'inputs':str(inputs),'fixture':str(fixture),'published':False}
(out/'native-initial-result.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print('Native owner test exit:',p.returncode,flush=True)
assert p.returncode==0
assert 'test result: ok. 1 passed; 0 failed;' in (out/'native-initial.log').read_text()
assert report['sourcesUnchanged'] and before==sha(runner)
