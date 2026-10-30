"""Replay declared native grammars through the complete CPU stream parser, no model execution."""
import json
from xolotl_trial_common import BASE,jump
r=json.loads((BASE/'xolotl-json-runtime-v6b-r2-stage.json').read_text())
code='''import hashlib,json,os,sys
from pathlib import Path
root=Path(@ROOT@);os.environ['PYTHONDONTWRITEBYTECODE']='1';os.environ['STRATA_TEXT_LOG']='0';sys.dont_write_bytecode=True
sys.path[:0]=[str(root/'python-deps-v2'),str(root/'source'),str(root/'source/tools/v41/serve')]
p=root/'source/tools/v41/serve/serve_ds41.py'
if hashlib.sha256(p.read_bytes()).hexdigest()!='b204c4af684f771a13b5889f62d9fe38b029f9f5af05e78747df10f406758b75':raise RuntimeError('retained strict adapter differs')
import serve_ds41 as D
tools=[dict(name='add_numbers',description='Add two integers.',parameters=dict(type='object',properties=dict(a=dict(type='integer'),b=dict(type='integer')),required=['a','b']))]
raw=D.INVOKE_OPEN+' name="add_numbers">'+json.dumps(dict(a=17,b=25))+D.INVOKE_CLOSE
rows=[]
for wrap in [False,True]:
 for forced in [False,True]:
  for width in [1,2,7,1000]:
   parser=D.F.OutputParser(thinking=False,tools=tools)
   text=(D.CALLS_OPEN+raw+D.CALLS_CLOSE) if wrap else raw
   events=[]
   if forced:
    prefix=D.INVOKE_OPEN+' name="add_numbers">';events+=parser.feed(prefix)
    text=json.dumps(dict(a=17,b=25))+D.INVOKE_CLOSE
   try:
    for i in range(0,len(text),width):events+=parser.feed(text[i:i+width])
    events+=parser.finish(reason='stop');calls=[dict(name=e.call.name,arguments=e.call.arguments) for e in events if e.kind=='tool_call']
    rows.append(dict(wrappers=wrap,forced=forced,width=width,passed=calls==[dict(name='add_numbers',arguments=dict(a=17,b=25))],calls=calls))
   except Exception as e:rows.append(dict(wrappers=wrap,forced=forced,width=width,passed=False,error=type(e).__name__+': '+str(e)))
print(json.dumps(dict(cases=rows,passed=all(x['passed'] for x in rows))))
'''.replace('@ROOT@',repr(r['root']))
o=jump(code);(BASE/'xolotl-strict-dsml-stream-v15-cpu-replay.json').write_text(json.dumps(o,indent=2)+'\n');print(json.dumps(o,indent=2))
