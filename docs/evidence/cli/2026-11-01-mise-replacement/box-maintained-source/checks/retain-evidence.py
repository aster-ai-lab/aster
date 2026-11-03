from pathlib import Path
import hashlib,json,platform,shutil,subprocess,time
root=Path.cwd()
out=root/'.tmp/plugins-box-maintained'
evidence=root/'docs/evidence/cli/2026-11-01-mise-replacement'
storage=evidence/'box-maintained-source'
assert not storage.exists()
storage.mkdir()
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
checks=[]
for name,argv in [('plugins-diff-check',['git','-C','plugins','diff','--check']),('parent-scope-diff-check',['git','diff','--check','--','specs/031-cli-mise-replacement/tasks.md','docs/evidence/cli/2026-11-01-mise-replacement/OUTCOME.md'])]:
    p=subprocess.run(argv,capture_output=True,text=True,encoding='utf-8')
    (out/(name+'.log')).write_text(p.stdout+p.stderr,encoding='utf-8')
    checks.append({'name':name,'argv':argv,'exitCode':p.returncode})
    assert p.returncode==0
owned=json.loads((out/'owned-source-before.json').read_text())
assert all(sha(root/p)==v['sha256'] for p,v in owned.items())
native_sources=json.loads((out/'native-source-before.json').read_text())
assert all(sha(root/p)==value for p,value in native_sources.items())
final=json.loads((out/'final-validation.json').read_text())
assert sha(out/'aster-use-builder-current')==final['builderSha256']
assert sha(out/'aster-studio-native-tests')==final['nativeRunnerSha256']
files=[]
def retain(source,name):
    destination=storage/name
    destination.parent.mkdir(parents=True,exist_ok=True)
    shutil.copy2(source,destination)
    files.append({'path':destination.relative_to(evidence).as_posix(),'sha256':sha(destination),'length':destination.stat().st_size})
for name in ['repository-tests.log','repository-tests-final.log','build-result.json','builder-build.log','recipe.log','old-builder-isolated-retry.json','catalog-metadata-before.json','current-builder-isolated-retry.json','first-current-checks.json','first-recipe-current.log','verify-current-first-attempt.py','verify-current.py','recipe-current.log','current-checks.json','source-archive-audit.json','native-inputs.json','native-runner.json','native-management.log','native-management.json','native-source-before.json','owned-source-initial.json','owned-source-before.json','final-validation.json','inventory.json','plugins-diff-check.log','parent-scope-diff-check.log']:
    retain(out/name,Path('checks')/name)
for name in ['qualified/checks.json','qualified-current/checks.json','qualified-final/checks.json','qualified-final/aster-box-0.1.0-darwin-arm64.oci.json','qualified-final/repository/plugins/operations/aster/aster-box/toolchain.acl']:
    retain(out/name,Path('generated')/name)
for name in owned:
    retain(root/name,Path('source')/name)
retain(root/'specs/031-cli-mise-replacement/tasks.md',Path('source/specs/031-cli-mise-replacement/tasks.md'))
retain(out/'retain-evidence.py',Path('checks/retain-evidence.py'))
archive=json.loads((out/'source-archive-audit.json').read_text())
packet={'schema':'aster.box-maintained-source-evidence.v1','status':'progress','observedAtUnixSeconds':int(time.time()),'host':platform.platform(),'rustToolchain':'1.97.1','scope':'Box maintained-source authoring and qualified-input packaging','packageId':'aster/aster-box','target':'darwin-arm64','recipeVersion':'0.1.0','toolVersion':'3.2.7','authoringSources':owned,'sampledNativeSources':native_sources,'repositoryTests':{'argv':['python3','-m','unittest','discover','-s','plugins/tests','-v'],'passed':12,'failed':0},'sdkBuild':{'argv':['cargo','build','--locked','-p','aster-use','--bin','aster-use'],'exitCode':0,'retainedPreexistingWarning':'RuntimeUnitSpec unused import in crates/runtime/src/driver.rs'},'nativeValidation':final,'archiveAudit':archive,'diffChecks':checks,'inventory':json.loads((out/'inventory.json').read_text()),'nativeManagement':json.loads((out/'native-management.json').read_text()),'retainedFailures':['old SDK flat-source-path preparation failure and isolated replay','unsorted new catalog keywords rejected by current SDK and isolated replay; corrected metadata passes'],'pending':[],'published':False,'notClaimed':['Box VM execution','macOS corresponding-source publication qualification','current Studio rebuild','native GUI acceptance','other native targets','all original inventory migrated and published','complete mise replacement','release performance and final archive/sandbox/channel gates','release or channel publication'],'retainedFiles':files}
packet_path=evidence/'box-maintained-source-results.json'
assert not packet_path.exists()
packet_path.write_text(json.dumps(packet,indent=2)+'\n',encoding='utf-8')
reopened=json.loads(packet_path.read_text())
assert len(reopened['retainedFiles'])==42
for row in reopened['retainedFiles']:
    p=evidence/row['path']
    assert p.stat().st_size==row['length'] and sha(p)==row['sha256']
assert archive['aixSha256']==sha(out/'qualified-final/aster-box-0.1.0-darwin-arm64.aix')
assert reopened['nativeManagement']['results'][0]['aixArchiveSha256']=='sha256:'+archive['aixSha256']
progress=json.loads((out/'progress.json').read_text())
progress.update({'status':'increment_verified','pending':[],'evidence':str(packet_path.relative_to(root)),'nextRequiredScope':['Box real VM execution and macOS corresponding-source qualification','remaining original plugin inventory and consumers','all 24 mise capability families','native GUI and platform acceptance','final product release gates']})
(out/'progress.json').write_text(json.dumps(progress,indent=2)+'\n',encoding='utf-8')
print('Reopened evidence:',len(reopened['retainedFiles']),'files; final archive and lifecycle digest match; no owned pending process')
