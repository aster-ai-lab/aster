"""Locate whether the retained continuation drift also occurs with short inputs; never weakens the full1M gate."""
import ast
import concurrent.futures
import hashlib
import json
import threading
import time
from pathlib import Path

ROOT=Path(__file__).resolve().parent
PLAN=json.loads((ROOT/'probe-plan.json').read_text())
raw=(ROOT/'qualified-request-source.py').read_bytes()
if hashlib.sha256(raw).hexdigest()!=PLAN['request_source_sha256']:raise RuntimeError('exact native-tool SSE helper source differs')
tree=ast.parse(raw);main=[node for node in tree.body if isinstance(node,ast.Try)]
if len(main)!=1:raise RuntimeError('exact request helper module boundary differs')
assignments=[node for node in main[0].body if isinstance(node,ast.Assign) and any(isinstance(target,ast.Name) and target.id=='cases' for target in node.targets)]
if len(assignments)!=1:raise RuntimeError('exact two public1024-output prompts differ')
cases=eval(compile(ast.Expression(assignments[0].value),'frozen-public-cases','eval'),{'dict':dict})
if [case['name'] for case in cases]!=['rust-lru-1024','distributed-cache-1024']:raise RuntimeError('closed public prompt set differs')
helper=compile(ast.Module(body=[node for node in tree.body if not isinstance(node,ast.Try)],type_ignores=[]),'frozen-native-SSE-helper','exec')
reference=json.loads((Path(PLAN['reference_root'])/'request-result.json').read_text())
if reference['phase']!='passed' or not reference['native_tool_roundtrip_passed']:raise RuntimeError('exact completed single-request reference required')
wanted=reference['reference_output_sha256']
reference_text={case['name']:next(row['content'] for row in reference['tests'] if row.get('case')==case['name']) for case in cases}
STATE=dict(unix=time.time(),phase='preflight',tests=[],pairs=[],full1m_input_tested=False,full1m_concurrency_qualified=False,cause_proven=False,
           scope='Eight actual short-input1024-output requests: serial warm2, same-prompt concurrent2, mixed concurrent2, serial post2; exact reference gate retained')


def save():
    temp=ROOT/'probe-result.tmp';temp.write_text(json.dumps(STATE,indent=2)+'\n');temp.replace(ROOT/'probe-result.json')


def ask(name,case,barrier=None):
    root=ROOT/name;root.mkdir(mode=0o700)
    (root/'request-plan.json').write_text(json.dumps(dict(service_root=PLAN['service_root'],base_url=PLAN['base_url'],profile='short_concurrency_probe_v81'))+'\n')
    ns=dict(__file__=str(root/'request-worker.py'),__name__='qualified_helper')
    exec(helper,ns)
    if barrier is not None:barrier.wait(timeout=10)
    row=ns['ask'](name,dict(messages=[dict(role='user',content=case['prompt'])],max_tokens=1024))
    if row['usage']['completion_tokens']!=1024 or row['finish_reason']!='length' or row['tool_calls'] or row['usage']['prompt_tokens']>=512:
        raise RuntimeError('short concurrency request did not finish the matched1024-token output')
    row.update(case=case['name'],same_as_reference=row['output_sha256']==wanted[case['name']])
    text=reference_text[case['name']]
    row['first_changed_character']=None if row['same_as_reference'] else next((i for i,(a,b) in enumerate(zip(row['content'],text)) if a!=b),min(len(row['content']),len(text)))
    return row


try:
    save();STATE['phase']='serial_warm';save()
    for i,case in enumerate(cases):
        row=ask('serial-warm-'+str(i),case);STATE['tests'].append(row);save()
        if not row['same_as_reference']:raise RuntimeError('short serial warm reference differs before concurrency')
    for name,group in [('same',[cases[0],cases[0]]),('mixed',cases)]:
        STATE.update(phase='concurrent',current_pair=name);save();barrier=threading.Barrier(2);started=time.monotonic()
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            futures=[pool.submit(ask,'concurrent-'+name+'-'+str(i),case,barrier) for i,case in enumerate(group)]
            rows=[future.result() for future in futures]
        wall=time.monotonic()-started
        STATE['tests'].extend(rows)
        STATE['pairs'].append(dict(pair=name,requests=2,completion_tokens=2048,wall_seconds=wall,api_wall_completion_tok_s=2048/wall,
                                   exact_reference_matched=all(row['same_as_reference'] for row in rows),throughput_qualified=all(row['same_as_reference'] for row in rows)))
        save()
    STATE['phase']='serial_post';save()
    for i,case in enumerate(cases):STATE['tests'].append(ask('serial-post-'+str(i),case));save()
    STATE.update(serial_warm_matched=all(row['same_as_reference'] for row in STATE['tests'][:2]),
                 serial_post_matched=all(row['same_as_reference'] for row in STATE['tests'][-2:]),
                 concurrent_reference_matched=all(row['same_as_reference'] for row in STATE['tests'][2:6]),finished_unix=time.time())
    STATE['phase']='passed' if all(row['same_as_reference'] for row in STATE['tests']) else 'failed'
    if STATE['phase']=='failed':STATE['error']='Exact short-input concurrent/serial complete-output identity differs; original full1M failure remains retained'
    save()
except BaseException as error:
    STATE.update(phase='failed',error=type(error).__name__+': '+str(error),finished_unix=time.time());save();raise
