from pathlib import Path
import datetime, hashlib, json, os, subprocess, sys, time
root=Path.cwd();out=root/'docs/evidence/code/2026-11-02-summary-input-retention';label=sys.argv[1]
registration=Path(sys.argv[2]) if len(sys.argv)>2 else out/'qualification-preregistration.json'
frozen=json.loads((out/(label+'-inputs.json')).read_text());reg=json.loads(registration.read_text())
critical=json.loads((out/(label+'-live/attempt-status.json')).read_text())
assert len(critical)==2 and all(r['status']=='FINISHED' and r['exitCode']==0 for r in critical), 'both original critical cases must pass'
for member in frozen['members'].values():assert hashlib.sha256(Path(member['path']).read_bytes()).hexdigest()==member['sha256']
assert reg['supplementaryWhenBothPass'][0][0]=='unseen-records'
fresh=Path(reg['supplementaryWhenBothPass'][0][1]);fresh_reg=fresh/'preregistration.json'
assert hashlib.sha256(fresh_reg.read_bytes()).hexdigest()==reg['freshInputRegistrationSha256']
for name,digest in json.loads(fresh_reg.read_text())['inputs'].items():assert hashlib.sha256((fresh/name).read_bytes()).hexdigest()==digest
assert hashlib.sha256((root/'.aster/config.acl').read_bytes()).hexdigest()==reg['configSha256']
attempts=out/(label+'-supplementary');attempts.mkdir();env=os.environ.copy()
output_limit = reg.get('outputTokenLimit', 4096)
assert type(output_limit) is int and output_limit in (4096, 8192)
env['ASTER_LH_OUTPUT_TOKENS'] = str(output_limit)
env.update(ASTER_CONFIG_FILE=str(root/'.aster/config.acl'),ASTER_LIVE_MODEL=reg['model'],ASTER_LH_EVIDENCE_DIR=str(attempts),ASTER_LH_CANDIDATE_MANIFEST=str(out/(label+'-pair.json')))
cases=[('unseen-records',None,'deloop_generalization_real_llm','opaque_selected_records_survive_a_real_long_run'),('native',2401,'long_horizon_real_llm','real_native_file_workflow_multiple_user_turns')]+[('rust',seed,'long_horizon_real_llm','real_rust_multifile_repair_binds_current_tests_and_independent_guest_oracle') for seed in [2401,2402,2403,2404,2405]]
assert reg['supplementaryWhenBothPass']==[[kind,str(fresh.resolve()) if seed is None else seed] for kind,seed,_,_ in cases]
rows=[]
for kind,seed,member,test in cases:
    assert hashlib.sha256((root/'.aster/config.acl').read_bytes()).hexdigest()==reg['configSha256']
    child=env.copy()
    if seed is None:child['ASTER_DELOOP_HOLDOUT_INPUTS']=str(fresh.resolve())
    else:child.update(ASTER_LH_FIXTURE_SEED=str(seed),ASTER_TEST_OCI_ASSETS=reg['compilerAssets'])
    command=[frozen['members'][member]['path'],'--ignored','--exact',test,'--nocapture','--test-threads=1'];row=dict(case=kind,seed=seed,attempt=1,status='RUNNING',startedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),command=command);rows.append(row)
    (attempts/'attempt-status.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps(dict(case=kind,seed=seed,status='RUNNING')),flush=True);started=time.monotonic()
    suffix='' if seed is None else '-seed-'+str(seed)
    with (attempts/(kind+suffix+'-attempt-1.log')).open('w') as log:r=subprocess.run(command,env=child,stdout=log,stderr=subprocess.STDOUT)
    row.update(status='FINISHED',exitCode=r.returncode,outerSeconds=time.monotonic()-started,finishedAt=datetime.datetime.now(datetime.timezone.utc).isoformat());(attempts/'attempt-status.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps({k:row[k] for k in ['case','seed','status','exitCode','outerSeconds']}),flush=True)
