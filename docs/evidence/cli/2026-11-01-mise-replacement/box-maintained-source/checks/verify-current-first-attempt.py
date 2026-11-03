from pathlib import Path
import hashlib,json,os,shutil,subprocess,time,zipfile

root=Path.cwd()
out=root/'.tmp/plugins-box-maintained'
sha=lambda path:hashlib.sha256(path.read_bytes()).hexdigest()
sources=json.loads((out/'owned-source-before.json').read_text())
native_sources=json.loads((out/'native-source-before.json').read_text())
assert all(sha(root/p)==row['sha256'] for p,row in sources.items())
assert all(sha(root/p)==value for p,value in native_sources.items())
builder=out/'aster-use-builder-current'
assert not builder.exists()
shutil.copy2(root/'target/debug/aster-use',builder)
builder_sha=sha(builder)
checks=[]

def run(name,argv,environment=None):
    start=time.monotonic()
    with (out/(name+'.log')).open('w',encoding='utf-8') as log:
        result=subprocess.run(argv,stdout=log,stderr=subprocess.STDOUT,env=environment)
    checks.append({'name':name,'argv':argv,'exitCode':result.returncode,'seconds':time.monotonic()-start})
    (out/'current-checks.json').write_text(json.dumps(checks,indent=2)+'\n',encoding='utf-8')
    print(name,result.returncode,flush=True)
    assert result.returncode==0

run('recipe-current',['python3',str(root/'plugins/recipes/aster-box/darwin-arm64.py'),str(root/'.tmp/plugin-registry/box-aix-candidate/aster-box-darwin-arm64.tar.gz'),'--aster-use-bin',str(builder),'--out',str(out/'qualified-current')])
staged=out/'qualified-current/repository/plugins/operations/aster/aster-box'
aix=out/'qualified-current/aster-box-0.1.0-darwin-arm64.aix'
with zipfile.ZipFile(aix) as archive:
    paths={p.relative_to(staged).as_posix():p for p in staged.rglob('*') if p.is_file()}
    assert set(archive.namelist())==set(paths)
    assert all(archive.read(name)==p.read_bytes() for name,p in paths.items())
    members=[{'path':name,'sha256':sha(p),'length':p.stat().st_size} for name,p in sorted(paths.items())]
assert sha(staged/'payload/aster-box-darwin-arm64.tar.gz')=='73e341931c6d8e3171b114a1ccb3694895cb1fb3d949c96bbb3256f510d47ef7'
for p in (root/'plugins/plugins/operations/aster/aster-box').iterdir():
    assert p.read_bytes()==(staged/p.name).read_bytes()
(out/'source-archive-audit.json').write_text(json.dumps({'aixSha256':sha(aix),'aixLength':aix.stat().st_size,'allStagedFilesMatchArchive':True,'allCanonicalFilesMatchStagedSource':True,'runtimeUnchangedFromQualifiedCandidate':True,'members':members},indent=2)+'\n',encoding='utf-8')
runner=out/'aster-studio-native-tests'
runner_before=sha(runner)
environment={**os.environ,'ASTER_TEST_DEPENDENCY_AIX_INPUTS':str(out/'native-inputs.json'),'ASTER_TEST_DEPENDENCY_AIX_EVIDENCE':str(out/'native-management.json')}
run('native-management',[str(runner),'capabilities::toolchain::tests::actual_dependency_aix_candidates_are_queryable_executable_and_removable','--ignored','--exact','--nocapture'],environment)
assert 'test result: ok. 1 passed; 0 failed;' in (out/'native-management.log').read_text()
assert sha(runner)==runner_before
assert sha(builder)==builder_sha
assert all(sha(root/p)==row['sha256'] for p,row in sources.items())
assert all(sha(root/p)==value for p,value in native_sources.items())
(out/'final-validation.json').write_text(json.dumps({'status':'passed','scope':'Box maintained-source package preparation and retained desktop-owner lifecycle','builderSha256':builder_sha,'nativeRunnerSha256':runner_before,'sourcesUnchanged':True,'binariesUnchanged':True,'checks':checks,'published':False,'nativeVmExecutionAccepted':False,'currentStudioRebuildClaimed':False},indent=2)+'\n',encoding='utf-8')
print('Box maintained-source verification passed',flush=True)
