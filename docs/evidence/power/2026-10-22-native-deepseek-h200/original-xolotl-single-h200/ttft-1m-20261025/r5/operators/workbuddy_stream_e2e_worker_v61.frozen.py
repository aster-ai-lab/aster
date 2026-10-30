"""Exercise two user turns in one actual WorkBuddy client process, preserving its in-memory conversation."""
import json,os,subprocess,threading,time,uuid
from pathlib import Path
BASE=Path(__file__).resolve().parent;P=json.loads((BASE/'workbuddy-stream-e2e-v61-plan.json').read_text());OUT=BASE/'workbuddy-stream-e2e-v61-result.json'
STATE=dict(phase='starting',unix=time.time(),client='installed WorkBuddy lite-wb, one stream-json conversation',gui_test_passed=False,base_url=P['base_url'],tests=[])
def save():
 p=OUT.with_suffix('.tmp');p.write_text(json.dumps(STATE,ensure_ascii=False,indent=2)+'\n');p.replace(OUT)
child=None;timer=None
try:
 save();env=dict(os.environ);env['CODEBUDDY_FORCE_LITE_WB_BUNDLE']='1'
 args=[P['node'],P['entry'],'--model','custom-local:'+P['model'],'--print','--input-format','stream-json','--output-format','stream-json','--include-partial-messages','--verbose','--no-session-persistence','--permission-mode','plan','--max-turns','2','--session-id','aster-stream-e2e-'+uuid.uuid4().hex]
 with (Path(P['private_output'])/'stdout.jsonl').open('xb') as raw,(Path(P['private_output'])/'stderr.log').open('xb') as err:
  child=subprocess.Popen(args,cwd=P['cwd'],env=env,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
  timer=threading.Timer(210,child.terminate);timer.start();texts=['hi。请只回复 hi，不调用工具、不读取或修改文件。','再回复 hi，不调用工具、不读取或修改文件。'];turn=0;t=time.time();first=None;answer='';events=0
  def send(text):
   child.stdin.write((json.dumps(dict(type='user',message=dict(role='user',content=text)),ensure_ascii=False)+'\n').encode());child.stdin.flush()
  send(texts[0]);STATE.update(phase='running',client_pid=child.pid,current_turn=1,started_unix=t);save()
  for line in iter(child.stdout.readline,b''):
   raw.write(line);raw.flush()
   if len(line)>4*1024**2:raise RuntimeError('WorkBuddy event bound exceeded')
   try:x=json.loads(line)
   except (ValueError,UnicodeError):continue
   events+=1
   if x.get('type')=='stream_event':
    part=x.get('event',{}).get('delta',{}).get('text') or ''
    if part:
     answer+=part
     if first is None:first=time.time()
   elif x.get('type')=='assistant' and not answer:
    answer=''.join(z.get('text','') for z in x.get('message',{}).get('content',[]) if isinstance(z,dict) and z.get('type')=='text')
    if answer and first is None:first=time.time()
   elif x.get('type')=='result':
    result={k:x.get(k) for k in ['subtype','is_error','result','duration_ms','duration_api_ms','num_turns','usage']};passed=not x.get('is_error') and answer.strip()=='hi'
    STATE['tests'].append(dict(turn=turn+1,passed=passed,seconds=time.time()-t,time_to_first_output_seconds=first-t if first else None,reply=answer[:2048],result=result));save()
    if not passed:raise RuntimeError('actual WorkBuddy conversation turn failed')
    turn+=1
    if turn==len(texts):child.stdin.close();break
    t=time.time();first=None;answer='';send(texts[turn]);STATE.update(current_turn=turn+1,started_unix=t);save()
   STATE.update(events=events,elapsed_seconds=time.time()-t);save()
  rc=child.wait(timeout=15)
  if len(STATE['tests'])!=2 or rc!=0:raise RuntimeError('WorkBuddy stream conversation did not finish both turns, exit='+str(rc))
 STATE.update(phase='passed',client_exit_code=rc,finished_unix=time.time());save()
except BaseException as e:
 if child is not None and child.poll() is None:child.terminate();child.wait(timeout=10)
 STATE.update(phase='failed',error=type(e).__name__+': '+str(e),finished_unix=time.time());save();raise
finally:
 if timer:timer.cancel()
