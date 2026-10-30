"""Actual installed WorkBuddy, fresh HI, default tool definitions; retain numeric/public result only."""
import json,os,subprocess,threading,time,uuid
from pathlib import Path
BASE=Path(__file__).resolve().parent
p=json.loads((BASE/'workbuddy-native-read-e2e-v60-plan.json').read_text())
out=BASE/'workbuddy-fresh-hi-v416-result.json'
if out.exists():raise RuntimeError('Fresh HI probe already has a receipt; inspect it')
s=dict(unix=time.time(),phase='starting',test='Fresh WorkBuddy session, literal HI, standard tool definitions',
 model=p['model'],base_url=p['base_url'],prompt='HI',default_tools_profile=True,no_session_persistence=True,
 GUI_tested=False,performance_optimization=False,events=0,tool_calls=0)
def save():
 t=out.with_suffix('.tmp');t.write_text(json.dumps(s,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');t.replace(out)
args=[p['node'],p['entry'],'--model','custom-local:'+p['model'],'--print','--output-format','stream-json',
 '--include-partial-messages','--verbose','--no-session-persistence','--permission-mode','dontAsk',
 '--max-turns','1','--session-id','aster-fresh-hi-second-'+uuid.uuid4().hex,'HI']
child=None;timer=None;started=time.monotonic();first=None;answer='';final=None;received=0
try:
 save()
 child=subprocess.Popen(args,cwd=p['cwd'],env=dict(os.environ,CODEBUDDY_FORCE_LITE_WB_BUNDLE='1'),
  stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,creationflags=subprocess.CREATE_NO_WINDOW)
 timer=threading.Timer(180,child.terminate);timer.start();s.update(phase='running',client_pid=child.pid);save()
 for line in iter(child.stdout.readline,b''):
  received+=len(line)
  if received>8*1024**2 or len(line)>2*1024**2:raise RuntimeError('Bounded CLI output exceeded')
  try:r=json.loads(line)
  except (ValueError,UnicodeError):continue
  s['events']+=1
  if r.get('type')=='stream_event':
   event=r.get('event',{});delta=event.get('delta',{})
   text=delta.get('text') or ''
   if text and first is None:first=time.monotonic();s['first_output_seconds']=first-started
  elif r.get('type')=='assistant':
   for b in r.get('message',{}).get('content',[]):
    if b.get('type')=='tool_use':s['tool_calls']+=1
    elif b.get('type')=='text':answer=b.get('text') or answer
  elif r.get('type')=='result':
   final={k:r.get(k) for k in ['subtype','is_error','duration_ms','duration_api_ms','num_turns','usage']}
   answer=r.get('result') or answer
  s['elapsed_seconds']=time.monotonic()-started;save()
 rc=child.wait(timeout=10)
 if rc or final is None or final.get('is_error') or not answer.strip() or s['tool_calls']:
  raise RuntimeError('Fresh HI must finish with nonempty text and no tool use')
 s.update(phase='passed',client_exit=rc,reply=answer[:1024],result=final,seconds=time.monotonic()-started,
  first_output_seconds=None if first is None else first-started,finished_unix=time.time());save()
 print(json.dumps(s,ensure_ascii=False,indent=2))
except BaseException as e:
 if child is not None and child.poll() is None:child.terminate();child.wait(timeout=10)
 s.update(phase='failed',error_type=type(e).__name__,error=str(e),seconds=time.monotonic()-started,finished_unix=time.time());save();raise
finally:
 if timer:timer.cancel()
