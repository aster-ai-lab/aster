"""Replay complete DSML through actual BPE and API incremental detokenizer, CPU only."""
import json
from xolotl_trial_common import BASE,jump
r=json.loads((BASE/'xolotl-json-runtime-v6b-r2-stage.json').read_text())
code='''import hashlib,json,os,sys,time
from pathlib import Path
root=Path(@ROOT@);os.environ['PYTHONDONTWRITEBYTECODE']='1';os.environ['STRATA_TEXT_LOG']='0';sys.dont_write_bytecode=True
sys.path[:0]=[str(root/'python-deps-v2'),str(root/'source'),str(root/'source/tools/v41/serve')]
source=root/'source/tools/v41/serve/serve_ds41.py'
if hashlib.sha256(source.read_bytes()).hexdigest()!='b204c4af684f771a13b5889f62d9fe38b029f9f5af05e78747df10f406758b75':raise RuntimeError('retained strict adapter differs')
import serve_ds41 as D
vocab=json.loads((root/'tokenizer/vocab.json').read_text());tokens=[None]*len(vocab)
for word,i in vocab.items():tokens[i]=word
tk=D.Ds41Tokenizer(tokens,(root/'tokenizer/merges.txt').read_text().split('\\n'),json.loads((root/'tokenizer/token_type.json').read_text()))
tools=[dict(name='add_numbers',description='Add two integers.',parameters=dict(type='object',properties=dict(a=dict(type='integer'),b=dict(type='integer')),required=['a','b']))]
expected=dict(a=17,b=25);rows=[]
json_body=json.dumps(expected)
parameter_body=D.P_OPEN+' name="a" string="false">17'+D.P_CLOSE+D.P_OPEN+' name="b" string="false">25'+D.P_CLOSE
for grammar,body in [('json',json_body),('native_parameters',parameter_body)]:
 for wrap in [False,True]:
  for forced in [False,True]:
   opener=D.INVOKE_OPEN+' name="add_numbers">'
   prefix=(D.CALLS_OPEN if wrap else '')+opener
   full=prefix+body+D.INVOKE_CLOSE+(D.CALLS_CLOSE if wrap else '')
   generated=(body+D.INVOKE_CLOSE+(D.CALLS_CLOSE if wrap else '')) if forced else full
   ids=tk.encode(generated,parse_special=True)
   for grouped in [1,2,7,1000]:
    parser=D.F.OutputParser(thinking=False,tools=tools,stream_tools=True)
    events=parser.feed(prefix) if forced else [];detok=D.S.Detokenizer(tk);pieces=[]
    try:
     decoded=[detok.push(i) for i in ids]
     for j in range(0,len(decoded),grouped):
      chunk=''.join(decoded[j:j+grouped]);pieces.append(chunk);events+=parser.feed(chunk)
     events+=parser.finish(reason='stop')
     calls=[dict(name=e.call.name,arguments=e.call.arguments) for e in events if e.kind=='tool_call']
     rows.append(dict(grammar=grammar,wrapped=wrap,forced=forced,grouped=grouped,bpe_tokens=len(ids),passed=calls==[dict(name='add_numbers',arguments=expected)],calls=calls,content=''.join(e.text or '' for e in events if e.kind=='content')))
    except Exception as e:rows.append(dict(grammar=grammar,wrapped=wrap,forced=forced,grouped=grouped,passed=False,error=type(e).__name__+': '+str(e),last_chunks=pieces[-8:]))
print(json.dumps(dict(unix=time.time(),model_execution=False,cases=rows,passed=all(x['passed'] for x in rows))))
'''.replace('@ROOT@',repr(r['root']))
compile(code,'public-dsml-bpe-replay','exec')
o=jump(code);(BASE/'xolotl-strict-dsml-bpe-v17-cpu-replay.json').write_text(json.dumps(o,indent=2)+'\n');print(json.dumps(dict(passed=o['passed'],cases=len(o['cases']),failed=[x for x in o['cases'] if not x['passed']]),indent=2))
