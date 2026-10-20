import hashlib,json,shutil
from pathlib import Path
def file_sha(path):
 h=hashlib.sha256()
 with path.open('rb') as f:
  for chunk in iter(lambda:f.read(1024*1024), b''):h.update(chunk)
 return h.hexdigest()
root=Path('target/evaluations/kylin-ime-gui')
out=Path('docs/evidence/work/2026-10-20-kylin-ime-hardening/gui-r4-observations');out.mkdir(exist_ok=True)
commit='9ccb0ea27ee2a19dd63506f3d5ed79be7a88856869b74a7fe74231d2378410af'
bundle=Path('target/packages/asterdream-0.1.2-kylin-x86_64-20261020-hardening-r4/ship/dev.a3s.AsterDream.flatpak')
bundle_hash=file_sha(bundle)
assert bundle_hash=='62840dfddea793ae675627be7247307cf46b2fdcb92e3c55705cbdb16918f4e1'
selected={
 'run-7fnFml':{'screenshots':['r4-modern-'+x for x in ['05','06','07','08','09','10','12']], 'texts':['ime-plain-control','你好abc好nihao','repeat-control'],'observer':True},
 'run-V6V5Mp':{'screenshots':['r4-old-'+x for x in ['05','06','07','08','09','11','13']], 'texts':['ime-plain-control','你好好nihao','nihao'],'observer':True},
 'run-Wfa1CS':{'screenshots':['r4-uninstrumented-'+x for x in ['05','06','07','09']], 'texts':['ime-plain-control','你好nihao'],'observer':False},
}
validation=[]
for run,settings in selected.items():
 p=root/run;dest=out/run;dest.mkdir(exist_ok=True)
 for name in ['actions.jsonl','startup-proof.json','gateway-wire.jsonl','xkb-repeat.jsonl','daemon-version.txt','daemon-mode.txt','expected-commit.txt','candidate-return-wire.json']:
  if (p/name).exists():shutil.copy2(p/name,dest/name)
 for shot in settings['screenshots']:shutil.copy2(p/(shot+'.png'),dest/(shot+'.png'))
 proof=json.loads((p/'startup-proof.json').read_text());env=proof['app_selected_environment']
 assert proof['expected_commit']==commit and not proof['routes'] and [x['ifname'] for x in proof['interfaces']]==['lo']
 assert not {'FCITX_ENABLE_SYNC_MODE','FCITX_DISABLE_SNOOPER','IBUS_DISABLE_SNOOPER'}&env.keys()
 assert ('LD_PRELOAD' in env)==settings['observer'] and env['GTK_IM_MODULE']=='fcitx' and env['GTK_IM_MODULE_FILE']=='/app/lib/im/fcitx4/immodules.cache'
 events=[];errors=[]
 for f in (p/'app-state').rglob('*.log'):
  for line in f.read_text().splitlines():
   if line.startswith('[IME-DOM] '):events.append(json.loads(line[10:]))
   if '[Renderer.renderError]' in line:errors.append(line)
 assert not errors
 assert bool(events)==settings['observer']
 (dest/'dom-events.jsonl').write_text(''.join(json.dumps(x,ensure_ascii=False)+'\n' for x in events))
 (dest/'render-errors.json').write_text(json.dumps(errors)+'\n')
 wire=[json.loads(x) for x in (p/'gateway-wire.jsonl').read_text().splitlines()]
 streams=[x for x in wire if x.get('stream') is True]
 assert len(streams)==len(settings['texts'])
 checks=[]
 for text,row in zip(settings['texts'],streams):
  encoded=json.dumps(text,ensure_ascii=False,separators=(',',':')).encode();sha=hashlib.sha256(encoded).hexdigest()
  assert sha==row['last_user_content_sha256'] and len(encoded)==row['last_user_content_bytes']
  checks.append({'synthetic_text':text,'json_encoded_bytes':len(encoded),'sha256':sha,'wire_sequence':row['sequence'],'matches':True})
 (dest/'wire-content-checks.json').write_text(json.dumps(checks,ensure_ascii=False,indent=2)+'\n')
 candidate=json.loads((p/'candidate-return-wire.json').read_text());assert candidate['stream_count']==1
 persisted=[]
 for f in (p/'flatpak-profile/data/dev.a3s.studio').rglob('messages/*.json'):
  persisted.extend(json.loads(f.read_text()))
 assert [r['data']['parts'][0]['text'] for r in persisted if r['role']=='user']==settings['texts']
 replies=[r for r in persisted if r['role']=='assistant' and any(x.get('text')=='OFFLINE_IME_FIXTURE_OK' for x in r['data']['parts'])]
 assert len(replies)==len(streams)
 captures=[e for e in events if e.get('phase')=='window-capture']
 keys=[e for e in captures if e.get('type')=='keydown']
 if events:assert all(e['targetTag']=='DIV' and 'composer-tiptap' in e['targetClass'] for e in keys)
 actions=[json.loads(x) for x in (p/'actions.jsonl').read_text().splitlines()]
 states=[x['stdout'].strip() for x in actions if x['action']==['im-state']];assert states and all(x=='2' for x in states)
 row={'run':run,'app_pid':proof['app_pid'],'expected_commit':commit,'default_ime_environment':True,'observer':settings['observer'],'dom_event_records':len(events),'renderer_errors':len(errors),'stream_requests':len(streams),'nonstream_title_requests':sum(x.get('stream') is False for x in wire),'candidate_stream_requests':candidate['stream_count'],'ime_state_checks':states,'persisted_reply_count':len(replies),'visible_reply_screenshots_manually_reviewed':settings['screenshots'],'wire_content_checks':checks}
 if run=='run-7fnFml':
  ends=[e for e in captures if e.get('type')=='compositionend'];normal=[e for e in keys if e.get('key')=='Enter']
  row['natural_second_enter_after_composition_ms']=normal[1]['timeStamp']-ends[-1]['timeStamp']
  hold=normal[2:];assert hold[0]['repeat'] is False and all(e['repeat'] is True for e in hold[1:])
  row['held_enter']={'first_repeat':hold[0]['repeat'],'later_repeat_true_count':len(hold)-1,'first_repeat_delay_ms':hold[1]['timeStamp']-hold[0]['timeStamp'],'stream_requests_added':1}
 if run=='run-V6V5Mp':
  ends=[e for e in captures if e.get('type')=='compositionend'];normal=[e for e in keys if e.get('key')=='Enter'];quick=[x for x in actions if x['action']==['quick-confirm-send']][0]
  row['quick_second_enter_after_composition_ms']=normal[-1]['timeStamp']-ends[-1]['timeStamp']
  assert 250<=row['quick_second_enter_after_composition_ms']<=400
  assert streams[-1]['time_ns']>quick['monotonic_ns_before_command'][2]
  assert not any(quick['monotonic_ns_before_command'][0]<x['time_ns']<quick['monotonic_ns_before_command'][2] for x in streams)
  row['quick_return_commands']=quick
  row['quick_post_after_second_physical_keydown']=True
 validation.append(row)
shutil.copy2(root/'r4-cleanup.json',out/'cleanup.json');assert not any(x['app_pid_still_present'] for x in json.loads((out/'cleanup.json').read_text()))
old_archive=Path('target/evaluations/kylin-ime-native-review/fcitx-4.2.9.6.tar.xz');old_sha=file_sha(old_archive);assert old_sha=='d55eee3f094037b5c32f0c5397d585fdf932cbc7d2091ed6f78fd7a9ae512c47'
identity={'app_commit':commit,'bundle':str(bundle),'bundle_sha256':bundle_hash,'old_daemon_archive':str(old_archive),'old_daemon_archive_sha256':old_sha,'old_reported_version':'4.2.9.5','old_fixture_qualification':'Built from official 4.2.9.6 archive; upstream version reports 4.2.9.5; not a Kylin distribution binary; fixture omits X11 candidate-window addon.'}
(out/'identity.json').write_text(json.dumps(identity,indent=2)+'\n')
(out/'result.json').write_text(json.dumps({'result':'passed_with_explicit_environment_qualifications','scope':'Isolated installed WebKit GUI input and instant offline reply; not physical Kylin/CUA/authentication/security acceptance','runs':validation},ensure_ascii=False,indent=2)+'\n')
fixture=out/'fixture';fixture.mkdir(exist_ok=True)
for name in ['run-r2.sh','driver.py','offline-gateway.py','collect-proof-r2.py','xkb-repeat.py','observe.py','R4-ACCEPTANCE-PLAN.md','archive-r4.py','bwrap-without-seccomp.py']:shutil.copy2(root/name,fixture/name)
observer=fixture/'readonly-observer';observer.mkdir(exist_ok=True)
for name in ['README.md','ime-dom-observer.c','ime-dom-observer.h','ime-dom-observer.js']:shutil.copy2(root/'readonly-observer'/name,observer/name)
shutil.copy2(root/'bwrap-experiment.jsonl',out/'bwrap-experiment-through-r4.jsonl')
manifest=[{'path':str(f.relative_to(out)),'bytes':f.stat().st_size,'sha256':hashlib.sha256(f.read_bytes()).hexdigest()} for f in sorted(out.rglob('*')) if f.is_file() and f.name!='manifest.json']
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'runs':len(validation),'screenshots':sum(len(x['screenshots']) for x in selected.values()),'manifest_files':len(manifest),'events':sum(x['dom_event_records'] for x in validation),'stream_requests':sum(x['stream_requests'] for x in validation),'renderer_errors':0}))
