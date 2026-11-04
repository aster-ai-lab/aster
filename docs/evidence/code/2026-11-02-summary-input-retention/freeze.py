from pathlib import Path
import hashlib, json, mmap, re, sys, uuid

root=Path.cwd()
sys.path.insert(0, str(root/'crates/code/acp/tests/long_horizon'))
from timing_runner import Recorder
out=root/'docs/evidence/code/2026-11-02-summary-input-retention'
label=sys.argv[1]
timing=Recorder(out/(label+'-freeze-phase-timing.jsonl'))
import atexit
atexit.register(timing.close)
timing.run_capture(['python3','docs/evidence/code/2026-11-02-summary-input-retention/namespace.py',label+'-after-build'],cwd=root,phase='source_snapshot_after').check_returncode()
before=json.loads((out/(label+'-before-build-namespace.json')).read_text())
after=json.loads((out/(label+'-after-build-namespace.json')).read_text())
assert before==after, 'CLI source namespace changed during build'
rows=json.loads((out/(label+'-build-status.json')).read_text())
assert all(row['exitCode']==0 for row in rows)
build=root/'target/qualification/capacity-build-464359e5-e38f-44f5-81d2-b02797d264c5/debug'
artifacts={name:build/name for name in ['aster','aster-acp']}
tests=['deloop_real_llm','deloop_generalization_real_llm','long_horizon_real_llm','acp_conformance_stdio','shipped_tui_e2e','cursor_account_tui_e2e']
for line in (out/(label+'-live-tests-build.log')).read_text().splitlines():
    if 'Executable unittests src/lib.rs (' in line:
        artifacts['aster-acp-unit']=root/line.split('(',1)[1].rsplit(')',1)[0]
    for name in tests:
        if 'Executable tests/'+name+'.rs (' in line:
            artifacts[name]=root/line.split('(',1)[1].rsplit(')',1)[0]
assert set(artifacts)==set(['aster','aster-acp','aster-acp-unit',*tests]), artifacts
dest=root/'target/qualification'/('fallback-state-'+label+'-'+str(uuid.uuid4()))
dest.mkdir()
members={}
for name,source in artifacts.items():
    target=dest/name
    timing.run_capture(['cp','-c',str(source),str(target)],cwd=root,phase='frozen_member_copy').check_returncode()
    def h(path):
        with timing.phase('frozen_member_digest'):
            return hashlib.sha256(path.read_bytes()).hexdigest()
    assert h(source)==h(target)
    members[name]={'path':str(target),'sha256':h(target),'bytes':target.stat().st_size}
expected='aster-cli-pair-v1:'+after['gitSourceHash']+':aarch64-apple-darwin'
ids=[]
for name in ['aster','aster-acp']:
    result=timing.run_capture([members[name]['path'],'--build-id'],cwd=root,phase='frozen_build_id_query',timeout=600)
    result.check_returncode()
    value=result.stdout.decode().strip()
    ids.append(value)
tester_ids={}
for name in [*tests,'aster-acp-unit']:
    with timing.phase('tester_embedded_id_check'):
        with open(members[name]['path'],'rb') as stream:
            with mmap.mmap(stream.fileno(),0,access=mmap.ACCESS_READ) as mapped:
                found={part.decode() for part in re.findall(rb'aster-cli-pair-v1:[a-f0-9]{40}:aarch64-apple-darwin',mapped)}
    tester_ids[name]=sorted(found)
    if name in ['shipped_tui_e2e','cursor_account_tui_e2e','aster-acp-unit']:
        assert not found or found=={expected}, (name, found)
    else:
        assert found=={expected}, (name, found)
        ids.extend(sorted(found))
(out/(label+'-identity.json')).write_text(json.dumps({'expected':expected,'observed':ids,'testerEmbeddedIds':tester_ids,'ptyIdentity':'PTY/Cursor/unit runners use the same unchanged source interval and frozen executable digests; any embedded ID must match','members':members},indent=2)+'\n')
assert len(ids)==6 and all(value==expected for value in ids), ids
registration=Path(sys.argv[2]) if len(sys.argv)>2 else out/'qualification-preregistration.json'
reg=json.loads(registration.read_text())
registered_id=reg.get('sourceVariantRegistration', {}).get('expectedBuildId')
assert registered_id is None or registered_id==expected, 'source differs from registered candidate'
assert hashlib.sha256((root/'.aster/config.acl').read_bytes()).hexdigest()==reg['configSha256']
record={'buildId':expected,'members':members,'configSha256':reg['configSha256'],'sourceNamespace':label+'-after-build-namespace.json','scope':'Exact CLI source namespace and frozen binaries, no source drift during build; host/model/cache variation still remains.'}
(out/(label+'-inputs.json')).write_text(json.dumps(record,indent=2)+'\n')
(out/(label+'-pair.json')).write_text(json.dumps({'schema':'aster.code.lh.candidate.v1','build_id':expected,'aster':members['aster'],'acp':members['aster-acp']},indent=2)+'\n')
print(json.dumps({'status':'FROZEN','label':label,'buildId':expected}),flush=True)
timing.close()
