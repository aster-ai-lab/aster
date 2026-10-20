import hashlib,json,shutil
from pathlib import Path
root=Path('target/evaluations/kylin-ime-gui')
out=Path('docs/evidence/work/2026-10-20-kylin-ime-hardening/gui-r2-observations')
out.mkdir(exist_ok=True)
selected={
 'run-O2FBdB':['r2-none-05','r2-none-06'],
 'run-wCZiI4':['r2-live-05'],
 'run-o97yDH':['r2-sync-08'],
 'run-dTuzP4':['r2-snooper-07','r2-snooper-08','r2-snooper-09','r2-snooper-10','r2-snooper-11','r2-snooper-12','r2-snooper-13'],
 'run-t8QUV5':['r2-repeat-off-03'],
 'run-vzqGQ5':['r2-nodaemon-04'],
}
summary=[]
for run,shots in selected.items():
 p=root/run; dest=out/run; dest.mkdir(exist_ok=True)
 for name in ['actions.jsonl','startup-proof.json','gateway-wire.jsonl','xkb-repeat.jsonl','daemon-version.txt','daemon-mode.txt','expected-commit.txt','wire-content-checks.json']:
  if (p/name).exists():shutil.copy2(p/name,dest/name)
 for name in shots:
  shutil.copy2(p/(name+'.png'),dest/(name+'.png'))
 events=[]; crash=[]
 for f in (p/'app-state').rglob('*.log'):
  for line in f.read_text().splitlines():
   if line.startswith('[IME-DOM] '):events.append(json.loads(line[len('[IME-DOM] '):]))
   if '[Renderer.renderError]' in line:crash.append(line)
 (dest/'dom-events.jsonl').write_text(''.join(json.dumps(e,ensure_ascii=False)+'\n' for e in events))
 (dest/'render-errors.json').write_text(json.dumps(crash,ensure_ascii=False,indent=2)+'\n')
 captures=[e for e in events if e.get('phase')=='window-capture']
 enter=[e for e in captures if e.get('key')=='Enter']
 wire=[json.loads(x) for x in (p/'gateway-wire.jsonl').read_text().splitlines()] if (p/'gateway-wire.jsonl').exists() else []
 persisted=any('OFFLINE_IME_FIXTURE_OK' in f.read_text() for f in (p/'flatpak-profile/data/dev.a3s.studio').rglob('messages/*.json'))
 summary.append({'run':run,'dom_event_count':len(events),'renderer_error_count':len(crash),'enter_capture_events':[{'sequence':e['sequence'],'type':e['type'],'repeat':e.get('repeat'),'timeStamp':e['timeStamp'],'keyCode':e['keyCode'],'isComposing':e['isComposing'],'viewComposing':e['viewComposing']} for e in enter], 'stream_requests':sum(x.get('stream') is True for x in wire),'nonstream_requests':sum(x.get('stream') is False for x in wire),'offline_reply_persisted':persisted})
(out/'validation.json').write_text(json.dumps({'installed_commit':'d588d6d5e186e0b960198af3dd75956ea4cd890367e47d4dcccf519b5a369bc8','overall_gui_acceptance':'failed_default_async_snooper_off','runs':summary},ensure_ascii=False,indent=2)+'\n')
fixtures=out/'fixture';fixtures.mkdir(exist_ok=True)
for name in ['run-r2.sh','run-r2-snooper.sh','driver.py','offline-gateway.py','collect-proof-r2.py','xkb-repeat.py','observe.py','R2-ACCEPTANCE-PLAN.md','archive-r2.py','bwrap-without-seccomp.py']:
 shutil.copy2(root/name,fixtures/name)
shutil.copy2(root/'bwrap-experiment.jsonl',out/'bwrap-experiment-through-r2.jsonl')
observer=fixtures/'readonly-observer';observer.mkdir(exist_ok=True)
for name in ['README.md','ime-dom-observer.c','ime-dom-observer.h','ime-dom-observer.js']:
 shutil.copy2(root/'readonly-observer'/name,observer/name)
manifest=[{'path':str(f.relative_to(out)),'bytes':f.stat().st_size,'sha256':hashlib.sha256(f.read_bytes()).hexdigest()} for f in sorted(out.rglob('*')) if f.is_file() and f.name!='manifest.json']
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'runs':len(summary),'files':len(manifest),'screenshots':sum(len(x) for x in selected.values()),'render_errors':sum(x['renderer_error_count'] for x in summary)}))
