"""Actual installed WorkBuddy performs one public WebSearch and answers through the repaired cluster endpoint."""
import json,os,subprocess,threading,time,uuid
from pathlib import Path

BASE=Path(__file__).resolve().parent
plan=json.loads((BASE/'workbuddy-native-read-e2e-v60-plan.json').read_text())
private=BASE/'workbuddy-repeated-search-private-v287';private.mkdir(exist_ok=False)
out=BASE/'workbuddy-repeated-search-e2e-v287-result.json'
state=dict(unix=time.time(),phase='starting',client='installed WorkBuddy CLI',gui_test_passed=False,
    base_url='http://10.102.93.68/v1',model=plan['model'],tools_profile=['WebSearch'],no_session_persistence=True,
    public_search_only=True,search_tool_roundtrip_passed=False,raw_output_publication_permitted=False)
def save():
 p=out.with_suffix('.tmp');p.write_text(json.dumps(state,indent=2)+'\n');p.replace(out)
prompt=('这是受控的重复调用保护测试。请按顺序逐轮执行四次 WebSearch，每轮只有一次调用，'
        '必须等上一轮结果返回后再进行下一轮。四次 query 字段必须完全相同，内容固定为'
        '“Rust 编程语言 官方网站 rust-lang.org”。即使工具返回错误也继续下一轮，'
        '不要并行调用，不要调用其它工具。最终严格只回复 COMPLETED=<实际执行次数>，不要附加解释。')

args=[plan['node'],plan['entry'],'--model','custom-local:'+plan['model'],'--print','--output-format','stream-json',
    '--include-partial-messages','--verbose','--no-session-persistence','--permission-mode','dontAsk',
    '--tools','WebSearch','--allowedTools','WebSearch','--max-turns','10','--session-id','aster-repeated-search-e2e-'+uuid.uuid4().hex,prompt]
child=timer=None;calls=[];answer='';final=None;received=0;events=0;first=None;start=time.monotonic()
try:
 save()
 with (private/'stdout.jsonl').open('xb') as raw,(private/'stderr.log').open('xb') as err:
  child=subprocess.Popen(args,cwd=plan['cwd'],env=dict(os.environ,CODEBUDDY_FORCE_LITE_WB_BUNDLE='1'),
      stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=err,creationflags=subprocess.CREATE_NO_WINDOW)
  timer=threading.Timer(300,child.terminate);timer.start();state.update(phase='running',client_pid=child.pid);save()
  for line in iter(child.stdout.readline,b''):
   received+=len(line)
   if len(line)>4*1024**2 or received>16*1024**2:raise RuntimeError('bounded actual WorkBuddy probe output exceeded')
   raw.write(line);raw.flush()
   try:r=json.loads(line)
   except (ValueError,UnicodeError):continue
   events+=1
   if r.get('type')=='stream_event':
    text=r.get('event',{}).get('delta',{}).get('text') or ''
    if text and first is None:first=time.monotonic()
   elif r.get('type')=='assistant':
    for block in r.get('message',{}).get('content',[]):
     if block.get('type')=='tool_use':
      if block.get('name')!='WebSearch':raise RuntimeError('unexpected tool outside the explicit search-only profile')
      arg=block.get('input') or {};calls.append(dict(name=block['name'],argument_keys=sorted(arg)))
     elif block.get('type')=='text':answer=block.get('text') or answer
   elif r.get('type')=='result':
    final={k:r.get(k) for k in ['subtype','is_error','duration_ms','duration_api_ms','num_turns','usage']}
    answer=r.get('result') or answer
   state.update(events=events,search_calls=len(calls),elapsed_seconds=time.monotonic()-start);save()
  code=child.wait(timeout=15)
 if code or not final or final.get('is_error') or len(calls)!=3 or answer.strip()!='COMPLETED=3':
  raise RuntimeError('WorkBuddy repeated-search workflow must stop at three completed calls and return a final answer')
 state.update(phase='passed',search_tool_roundtrip_passed=False,repeated_search_workflow_terminated=True,successful_authenticated_search_tested=False,search_calls=len(calls),reply=answer,
     result=final,client_exit_code=code,seconds=time.monotonic()-start,first_output_seconds=None if first is None else first-start,
     finished_unix=time.time());save();print(json.dumps(state,ensure_ascii=False,indent=2))
except BaseException as e:
 if child is not None and child.poll() is None:child.terminate();child.wait(timeout=10)
 state.update(phase='failed',error=type(e).__name__+': '+str(e),seconds=time.monotonic()-start,finished_unix=time.time());save();raise
finally:
 if timer:timer.cancel()
