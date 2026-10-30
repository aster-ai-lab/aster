"""Run only after native cold completion, public replay and final runtime checks."""
import gzip
import hashlib
import json
import statistics
from pathlib import Path

ROOT=Path(__file__).resolve().parent
EXPECTED='EARLY=violet-7391-copper;MIDDLE=silver-4826-orchid;LATE=amber-9153-lantern'
FIXTURE_SHA='539aadd60c25d9abfbfc4defab2a3c658b9abc9e78cd45355b3b0930bdd12489'

def read(relative):
    p=ROOT/relative
    assert p.stat().st_size<=8*1024*1024,relative
    return json.loads(p.read_text(encoding='utf-8-sig'))

rows=[]
for name,on in [('paired-off-baseline',False),('dspark-d4-tier0-chunk512',True),('dspark-d4-tier25-chunk512',True),('dspark-coldA-chunk256-indexer64m',True)]:
    q=read(name+'/quality.json');assert len(q)==5 and all(r['correct'] for r in q)
    d=[r for r in read(name+'/decode-trials.json') if not r['warmup']]
    assert len(d)==6
    for r in d:
        t=r['response']['timings'];assert t['predicted_n']==r['response']['usage']['completion_tokens']==128
        assert 0<=t.get('draft_n_accepted',0)<=t.get('draft_n',0)
        if on:assert t['draft_n']>0 and t['draft_n_accepted']>0
        else:assert t.get('draft_n',0)==0
    p=read(name+'/prefill-trials.json');assert len(p)==3
    for r in p:
        t=r['response']['timings']
        assert r['correct'] and r['response']['choices'][0]['finish_reason']=='stop'
        assert t['cache_n']==0 and t['prompt_n']==r['response']['usage']['prompt_tokens']
        assert r['ttft_seconds']>0 and r['sse_file']
    rows.append({'profile':name,'quality_passed':5,'warm_decode_trials':len(d),
                 'decode_tokens_per_second_median':statistics.median(r['response']['timings']['predicted_per_second'] for r in d),
                 'prefill_tokens_per_second_median':statistics.median(r['response']['timings']['prompt_per_second'] for r in p),
                 'sse_ttft_seconds_median':statistics.median(r['ttft_seconds'] for r in p),
                 'drafts_offered':sum(r['response']['timings'].get('draft_n',0) for r in d),
                 'drafts_accepted':sum(r['response']['timings'].get('draft_n_accepted',0) for r in d)})
with gzip.open(ROOT/'fixture/prompt.txt.gz','rb') as f:fixture=f.read(8*1024*1024+1)
assert len(fixture)==5357485 and hashlib.sha256(fixture).hexdigest()==FIXTURE_SHA
cold=read('native-one-million-optimized/result.json');t=cold['timings']
assert all(cold[k] for k in ['success','full_one_million_completed','retrieval_passed'])
assert cold['content'].strip()==EXPECTED and cold['finish_reason']=='stop'
assert cold['usage']['prompt_tokens']==t['prompt_n']==1000123 and t['cache_n']==0
assert cold['usage']['completion_tokens']==t['predicted_n']
assert 0<t['draft_n_accepted']<=t['draft_n']
before=read('native-one-million-optimized/identity-before.json')
after=read('native-one-million-optimized/identity-after.json');assert before==after
assert before['engine_env']['STRATA_INDEXER_TILE']=='67108864'
assert before['engine_env']['STRATA_SERVE_DSPARK']
resource=read('native-one-million-resource-summary.json')
assert resource['passed'] and resource['samples']>=250 and resource['cpu_throttled_max']==0
assert all(value==0 for value in resource['memory_events_max'].values())
baseline=read('baseline.json')['native_1m']['timings']
assert t['prompt_ms']<baseline['prompt_ms']*.95,'Cold improvement was below the declared 5% threshold'
assert rows[-1]['decode_tokens_per_second_median']>rows[0]['decode_tokens_per_second_median']*1.05
public=read('public-one-million-retrieval/result.json')
assert public['success'] and public['status']==200 and public['content'].strip()==EXPECTED
accounting=read('public-one-million-native-accounting.json')
assert accounting['success'] and accounting['request_delta']==1 and accounting['exact_prompt_tokens']==1000123
assert accounting['cached_tokens']>=1000123-256,'Public replay did not retain the warm prefix'
tools=read('final-workbuddy/workbuddy-tool-roundtrip.json')
assert tools['all_passed'] and len(tools['cases'])==6 and all(c['passed'] for c in tools['cases'])
requests=0
for c in tools['cases']:
    assert c['model']=='deepseek-v4.1-flash'
    if 'call_status' in c:assert c['call_status']==c['result_status']==200;requests+=2
    else:assert c['status']==200;requests+=1
assert requests==10
health=read('final-runtime-health.json');assert health['all_passed'] and all(health['checks'].values())
official=read('official-model-and-supplier-state-final.json')
assert official['model']['state']=='published' and official['model']['contextWindow']==1048576
assert official['supplier']['enabled'] and official['supplier']['routable']
catalog=read('final-public-model-catalog.json');assert catalog['passed'] and catalog['status']==200
assert {'auto','deepseek-v4.1-flash','platform/apofasi','platform/bge-m3','platform/bge-reranker-v2-m3','platform/pp-ocr-v6','prism-27b','prism-27b-alt'}<=set(catalog['ids'])
selected=read('final-selected-profile.json')
assert selected['all_passed'] and selected['same_v3_weights'] and selected['other_service_pids_unchanged']
assert selected['power_sha256']==before['power_sha256'] and selected['engine_sha256']==before['engine_sha256']
pre=read('foreground-preemption-native-structured/result.json')
assert pre['yielded_to_foreground'] and not pre['full_one_million_completed']
assert read('foreground-preemption-json-public/summary.json')['passed']
sampler=read('sampler-distribution-result.json')
assert sampler['passed'] and sampler['exit_code']==0 and sampler['distribution_cases']==36 and sampler['draws_per_distribution_case']==400000
assert sampler['source_sha256']==read('candidate-indexer-artifacts.json')['engine_source_after_sha256']
audit={'all_passed':True,'cohorts':rows,'short_decode_ratio':rows[-1]['decode_tokens_per_second_median']/rows[0]['decode_tokens_per_second_median'],
       'cold_prompt_tokens':1000123,'baseline_cold_prefill_seconds':baseline['prompt_ms']/1000,'optimized_cold_prefill_seconds':t['prompt_ms']/1000,
       'cold_prefill_speed_ratio':baseline['prompt_ms']/t['prompt_ms'],'optimized_1m_decode_tokens_per_second':t['predicted_per_second'],
       'one_million_retrieval_passed':True,'public_warm_replay_passed':True,'final_workbuddy_http_requests':requests,'official_healthy':True,'sampler_distribution_cases':36,
       'limitations':['Synthetic repeated archive retrieval; not all real code workloads','Compound profile and allocator state; no single-knob attribution','No global quality-equivalence or hardware-limit claim','Initial strict Hi literal check failed and remains recorded']}
(ROOT/'completion-audit.json').write_text(json.dumps(audit,indent=2)+'\n',encoding='utf-8')
print(json.dumps(audit,indent=2))
