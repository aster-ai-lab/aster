from pathlib import Path
import hashlib,json,platform,shutil,subprocess,time
root=Path.cwd()
out=root/'.tmp/box-plugin-owner'
base=root/'docs/evidence/cli/2026-11-01-mise-replacement'
storage=base/'box-native-consumer'
assert not storage.exists()
storage.mkdir()
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
sampled=json.loads((out/'source-before.json').read_text())
assert all(sha(root/p)==d for p,d in sampled.items())
final=json.loads((out/'native-final-result.json').read_text())
management=json.loads((out/'native-management-final.json').read_text())
assert final['exitCode']==0 and final['binariesUnchanged'] and final['sourcesUnchanged']
assert management['results'][0]['aixArchiveSha256']=='sha256:'+final['aixSha256']
consumer=management['results'][0]['nativeConsumer']
assert [c['exitCode'] for c in consumer['commands']]==[0,17]
assert all(c['inventoryEmpty'] for c in consumer['commands'])
assert all(management[k] for k in ['allCandidatesRemoved','runtimeRetainedUntilLeaseRelease','installedCatalogueQueried'])
fixture=root/'.tmp/box-plugin-vm-probe/alpine-arm64.oci.tar'
reference=root/'specs/031-cli-mise-replacement/contracts/box-native-vm-reference.json'
contract=json.loads(reference.read_text())
assert fixture.stat().st_size==contract['offlineFixtureLength'] and sha(fixture)==contract['offlineFixtureSha256']
aix=out/'qualified-final/aster-box-0.1.0-darwin-arm64.aix'
assert sha(aix)==final['aixSha256'] and aix.stat().st_size==final['aixLength']
assert sha(out/'studio-native-tests')==final['runnerSha256']
commands=[['git','-C','apps/studio','diff','--check','--','src-tauri/src/capabilities_toolchain_tests.rs','src-tauri/src/capabilities_toolchain_box_tests.rs'],['git','-C','plugins','diff','--check'],['git','diff','--check','--','specs/031-cli-mise-replacement/tasks.md','specs/031-cli-mise-replacement/contracts/capability-matrix.md','specs/031-cli-mise-replacement/contracts/box-native-vm-reference.json','docs/evidence/cli/2026-11-01-mise-replacement/OUTCOME.md']]
whitespace=[]
for argv in commands:
    p=subprocess.run(argv,capture_output=True,text=True,encoding='utf-8')
    whitespace.append({'argv':argv,'exitCode':p.returncode,'stdout':p.stdout,'stderr':p.stderr})
    assert p.returncode==0
(out/'diff-checks.json').write_text(json.dumps(whitespace,indent=2)+'\n',encoding='utf-8')
files=[]
def retain(source,name):
    target=storage/name
    target.parent.mkdir(parents=True,exist_ok=True)
    shutil.copy2(source,target)
    files.append({'path':target.relative_to(base).as_posix(),'length':target.stat().st_size,'sha256':sha(target)})
for name in ['before/capabilities_toolchain_tests.rs','helper.diff','source-before.json','focused-tests.log','root-format.log','root-format-final.log','scoped-format.log','studio-clippy.log','clippy-scope-review.json','workspace-dependencies.log','workspace-dependencies-python312.log','run-native.py','native-initial-result.json','native-initial.log','native-management-initial.json','final-package.py','final-package.log','native-final-result.json','native-final.log','native-management-final.json','native-inputs-final.json','native-default-result.json','native-default.log','native-management-default.json','final-source-archive-audit.json','diff-checks.json','retain-evidence.py','upstream-source/release.json','upstream-source/sources.json','qualified-final/checks.json','qualified-final/aster-box-0.1.0-darwin-arm64.oci.json','qualified-final/repository/plugins/operations/aster/aster-box/toolchain.acl']:
    retain(out/name,Path('checks')/name)
probe=root/'.tmp/box-plugin-vm-probe'
for name in ['probe.py','checks.json','accepted.json','image.json','fixture-export.json','kernel-inspection.json']:
    retain(probe/name,Path('diagnostic')/name)
changed=['apps/studio/src-tauri/src/capabilities_toolchain_tests.rs','apps/studio/src-tauri/src/capabilities_toolchain_box_tests.rs','plugins/plugins/operations/aster/aster-box/README.md','plugins/recipes/aster-box/darwin-arm64.py','plugins/recipes/aster-box/darwin-arm64.json','specs/031-cli-mise-replacement/contracts/box-native-vm-reference.json','specs/031-cli-mise-replacement/tasks.md','specs/031-cli-mise-replacement/contracts/capability-matrix.md']
for name in changed:retain(root/name,Path('source')/name)
packet={'schema':'aster.box-native-consumer-evidence.v1','status':'progress','observedAtUnixSeconds':int(time.time()),'host':platform.platform(),'rustToolchain':'1.97.1','previousGoalTurn':'progress: maintained Box source and verified its package lifecycle','scope':'Two basic Box MicroVM commands through the native installed-package owner','sourceSha256':{p:sha(root/p) for p in changed},'sampledNativeSourceSha256':sampled,'frozenReference':{'path':str(reference.relative_to(root)),'sha256':sha(reference),'contract':contract},'externalFixture':{'path':str(fixture.relative_to(root)),'length':fixture.stat().st_size,'sha256':sha(fixture),'retainedOutsideGit':True,'publicFixtureHostingClaimed':False},'finalAix':{'path':str(aix.relative_to(root)),'length':aix.stat().st_size,'sha256':sha(aix)},'checks':{'focusedTests':{'passed':6,'failed':0,'ignoredExternal':4},'initialNativeConsumer':json.loads((out/'native-initial-result.json').read_text()),'finalNativeConsumer':final,'finalDefaultNativeCaller':json.loads((out/'native-default-result.json').read_text()),'sdkPackageChecks':json.loads((out/'qualified-final/checks.json').read_text()),'clippy':json.loads((out/'clippy-scope-review.json').read_text()),'scopedFormatExitCode':0,'finalRootFormatExitCode':0,'workspaceDependencies':{'interpreter':'python3.12','passed':7,'failed':0},'whitespace':whitespace},'nativeManagement':management,'archiveAudit':json.loads((out/'final-source-archive-audit.json').read_text()),'retainedFailures':['initial default Python 3.9 import lacks tomllib; original dependency assertions pass under installed Python 3.12','initial root format differences in three parallel-development files; files later change outside this increment and identical command passes'],'cleanupNote':'The --rm success path removes the instance; extra rm attempts report 1 while independently queried inventories remain empty. No rendered-error branching is used.','originalHelperCallersPreserved':True,'published':False,'permanentInstallation':False,'pending':[],'priorEvidence':'box-maintained-source-results.json','notClaimed':['every Box operation or snapshot/service/network behavior','ACP/agent sandbox integration','macOS corresponding-source publication qualification','GUI acceptance','other native host platforms','public hosting or CI availability of the qualified offline fixture','all 49 original IDs migrated and published','complete mise replacement','release performance and final archive/sandbox/channel gates','whole Studio Rust-guideline compliance or audit of every warning','release or channel publication'],'retainedFiles':files}
path=base/'box-native-consumer-results.json'
assert not path.exists()
path.write_text(json.dumps(packet,indent=2)+'\n',encoding='utf-8')
v=json.loads(path.read_text())
for row in v['retainedFiles']:
    p=base/row['path']
    assert p.stat().st_size==row['length'] and sha(p)==row['sha256']
assert len(v['retainedFiles'])==46
matrix=(root/'specs/031-cli-mise-replacement/contracts/capability-matrix.md').read_text()
assert len([line for line in matrix.splitlines() if line.startswith('| C')])==24
tasks=(root/'specs/031-cli-mise-replacement/tasks.md').read_text()
assert all(f'- [ ] T{i:03}' in tasks for i in range(4,15))
progress=json.loads((out/'progress.json').read_text())
progress.update({'status':'increment_verified','pending':[],'evidence':str(path.relative_to(root)),'remainingRequiredFullScope':['other Box operations and native consumer integrations','macOS corresponding-source publication qualification','remaining original plugin migrations and consumers','all 24 mise capability families','native GUI and target-platform acceptance','final product archive and release gates']})
(out/'progress.json').write_text(json.dumps(progress,indent=2)+'\n',encoding='utf-8')
print('Reopened',len(v['retainedFiles']),'evidence files; final AIX/owner/fixture identities match; all 24 families and unchecked tasks retained')
