"""Add public begin/middle/end retrieval across a real prompt-read handoff."""
import ast,hashlib,json
from pathlib import Path
BASE=Path('D:/code/aster/target/h200-native')
s=(BASE/'xolotl_ttft_qualification_worker_v9.frozen.py').read_text().replace('import concurrent.futures,hashlib,json,os,socket,statistics,time,urllib.request','import concurrent.futures,hashlib,json,os,socket,statistics,sys,time,urllib.request')
needle="    STATE.update(phase='passed',finished_unix=time.time());save()"
extra='''    STATE['phase']='handoff_retrieval';save()
    service=Path(P['service_root'])
    sys.path[:0]=[str(service/'python-deps-v2'),str(service/'source'),str(service/'source/tools/v41/serve')]
    import serve_ds41 as D
    vocab=json.loads((service/'tokenizer/vocab.json').read_text());tokens=[None]*len(vocab)
    for word,i in vocab.items():tokens[i]=word
    tk=D.Ds41Tokenizer(tokens,(service/'tokenizer/merges.txt').read_text().split('\\n'),json.loads((service/'tokenizer/token_type.json').read_text()));template=D.Ds41Template()
    def count(t):return len(tk.encode(template.render([dict(role='user',content=t)],enable_thinking=False),parse_special=True))
    expected=dict(begin=731927,middle=408563,end=962814)
    prefix='Public record retrieval test. Ignore filler words.\\nBEGIN_RECORD_CODE: 731927\\nFiller:'
    middle='\\nMIDDLE_RECORD_CODE: 408563\\nFiller:'
    suffix='\\nEND_RECORD_CODE: 962814\\nReturn a plain JSON object with keys begin, middle, end and the integer record codes. No markdown or commentary.'
    def fixture(word):
        n=35700-count(prefix+middle+suffix)
        for _ in range(8):
            t=prefix+(' '+word)*(n//2)+middle+(' '+word)*(n-n//2)+suffix
            actual=count(t)
            if actual==35700:return t
            n+=35700-actual
        raise RuntimeError('public retrieval fixture token count differs')
    cat=fixture('cat');dog=fixture('dog')
    STATE['retrieval_fixtures']={w:dict(prompt_tokens=35700,sha256=hashlib.sha256(t.encode()).hexdigest()) for w,t in [('cat',cat),('dog',dog)]};save()
    fresh=record(ask('fresh-three-records',cat,64))
    if fresh['usage']['prompt_tokens']!=35700 or json.loads(fresh['content'])!=expected:raise RuntimeError('fresh three-position retrieval failed')
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        long=pool.submit(ask,'yielded-three-records',dog,64);time.sleep(.5)
        side=pool.submit(ask,'yielded-isolation-53','Calculate 19 + 34. Reply with only the integer answer.',16)
        sr=record(side.result());lr=record(long.result())
    if sr['content'].strip()!='53' or lr['usage']['prompt_tokens']!=35700 or json.loads(lr['content'])!=expected:raise RuntimeError('yielded three-position retrieval or request isolation failed')
    STATE['handoff_three_position_retrieval_passed']=True
'''
if s.count(needle)!=1:raise RuntimeError('qualification completion anchor differs')
s=s.replace(needle,extra+needle);ast.parse(s);p=BASE/'xolotl_ttft_qualification_worker_v15.frozen.py';p.write_text(s,newline='\n')
p=BASE/'launch_xolotl_ttft_qualification_v15.frozen.py';s=p.read_text().replace("BASE/'xolotl_ttft_qualification_worker_v9.frozen.py'", "BASE/'xolotl_ttft_qualification_worker_v15.frozen.py'");ast.parse(s);p.write_text(s,newline='\n')
print(json.dumps(dict(phase='frozen',retrieval_positions=3,prompt_tokens_each=35700,actual_handoff_plus_isolation=True)))
