"""Thirteen exact serial calls, forcing a real pool restore before long-output comparisons."""
import ast
import hashlib
import json
import time
from pathlib import Path
from xolotl_fullwindow_long_fixture_v54 import check_record_prefix

ROOT=Path(__file__).resolve().parent
plan=json.loads((ROOT/'request-plan.json').read_text())
raw=(ROOT/'request-only-source.py').read_bytes()
if hashlib.sha256(raw).hexdigest()!=plan['request_source_sha256']:
    raise RuntimeError('exact original SSE helper differs')
tree=ast.parse(raw)
if sum(isinstance(node,ast.Try) for node in tree.body)!=1:
    raise RuntimeError('exact request-only helper boundary differs')
ns=dict(__file__=str(ROOT/'request-only.py'),__name__='frozen_request_only')
exec(compile(ast.Module(body=[n for n in tree.body if not isinstance(n,ast.Try)],type_ignores=[]),
             'qualified-original-SSE-helper','exec'),ns)
state=ns['STATE'];save=ns['save'];ask=ns['ask']


def log_bytes():
    p=Path(plan['service_root'])/'engine.log'
    if p.is_symlink() or p.stat().st_uid!=10250 or p.stat().st_size>8*1024**2:
        raise RuntimeError('bounded public-only native diagnostics differ')
    return p.read_bytes()


def records(row,fixture,cached):
    usage=row['usage'];n=fixture['actual_prompt_tokens']
    if (row['status']!=200 or not row['done'] or row['finish_reason']!='length'
            or usage['prompt_tokens']!=n or usage['completion_tokens']!=1024
            or usage['total_tokens']!=n+1024):
        raise RuntimeError('complete exact natural-input/1024-output accounting differs')
    reused=usage.get('prompt_tokens_details',{}).get('cached_tokens')
    if type(reused) is not int or (reused!=0 if not cached else not n-8<=reused<=n):
        raise RuntimeError('exact cold/restored prefix accounting differs')
    check_record_prefix(row['content'])


try:
    state.update(schema='aster-xolotl-pool-long-output-v110',phase='preflight',profile=plan['profile'],
                 diagnostic_completed=False,cold_reference_qualified=False,cases=[],identity_failures=[],
                 actual1m_input_tested=False,formal_service_changed=False,performance_300_claimed=False,
                 scope='Exact8244/25671 public technical-document inputs, six1024-output calls and forced pool restores; configured1M, not actual1M input.')
    save();reference=None
    if plan.get('reference_root'):
        reference=json.loads((Path(plan['reference_root'])/'request-result.json').read_text())
        if (not reference.get('diagnostic_completed') or len(reference['tests'])!=13
                or not reference.get('cold_reference_qualified')):
            raise RuntimeError('completed structurally qualified cold reference required; its failed replay gate is retained')
    control=ask('short-control-42','Calculate 17 + 25. Reply with only the integer answer.',max_tokens=16)
    if control['content'].strip()!='42':raise RuntimeError('short arithmetic control failed')
    if [f['actual_prompt_tokens'] for f in plan['fixtures']]!=[8244,25671]:
        raise RuntimeError('exact natural long-output input targets differ')
    for fixture in plan['fixtures']:
        p=Path(plan['fixture_root'])/fixture['file'];raw=p.read_bytes()
        if p.is_symlink() or len(raw)!=fixture['prompt_bytes'] or hashlib.sha256(raw).hexdigest()!=fixture['prompt_sha256']:
            raise RuntimeError('exact frozen natural long-output fixture differs')
        text=raw.decode();case=fixture['case']
        state.update(phase='cold_natural_long',current_case=case);save()
        cold=ask('cold-'+case,text,max_tokens=1024,budget=300);records(cold,fixture,False)
        wanted=cold['output_sha256']
        if reference:
            wanted=next(r['output_sha256'] for r in reference['tests'] if r['name']==cold['name'])
        cold['matches_control_cold']=cold['output_sha256']==wanted
        if not cold['matches_control_cold']:state['identity_failures'].append(cold['name'])
        state['phase']='displacing_live_slots';save()
        for label,prompt,answer in [
            ('53','Calculate 19 + 34. Reply with only the integer answer.','53'),
            ('47','Reply with only the integer 47.','47'),
            ('89','Return only the integer 89.','89')]:
            row=ask('isolation-'+case+'-'+label,prompt,max_tokens=16)
            if row['content'].strip()!=answer:raise RuntimeError('interleaved isolation arithmetic differs')
        before=log_bytes();state['phase']='restoring_from_pool';save()
        restored=ask('restored-'+case,text,max_tokens=1024,budget=180);records(restored,fixture,True)
        delta=log_bytes()[len(before):].decode(errors='strict')
        expected=f"strata serve: conversation cache: restored {fixture['actual_prompt_tokens']} tokens (pool)"
        if expected not in delta:raise RuntimeError('the long request did not prove an actual full-boundary pool restore')
        seed_line=f"serve: conversation cache: DSpark seed restored 128 rows at {fixture['actual_prompt_tokens']}"
        seed_proof=seed_line in delta
        if seed_proof!=plan['pool_seed_expected']:
            raise RuntimeError('actual pool seed restoration does not match the explicit flag')
        restored.update(pool_restore_logged=True,pool_seed_restore_logged=seed_proof,
                        matches_control_cold=restored['output_sha256']==wanted)
        if not restored['matches_control_cold']:state['identity_failures'].append(restored['name'])
        state['phase']='repeating_restored_boundary';save()
        repeated=ask('repeated-'+case,text,max_tokens=1024,budget=180);records(repeated,fixture,True)
        repeated['matches_control_cold']=repeated['output_sha256']==wanted
        if not repeated['matches_control_cold']:state['identity_failures'].append(repeated['name'])
        state['cases'].append(dict(case=case,actual_input_tokens=fixture['actual_prompt_tokens'],
            cold_seconds=cold['seconds'],cold_ttft_seconds=cold['ttft_seconds'],
            restored_seconds=restored['seconds'],restored_ttft_seconds=restored['ttft_seconds'],
            repeated_seconds=repeated['seconds'],pool_restore_logged=True,pool_seed_restore_logged=seed_proof,
            restored_matches_control_cold=restored['matches_control_cold'],
            repeated_matches_control_cold=repeated['matches_control_cold']))
        save()
    state.update(diagnostic_completed=True,cold_reference_qualified=True,
                 phase='passed' if not state['identity_failures'] else 'failed',finished_unix=time.time())
    if state['identity_failures']:state['error']='Complete greedy output differs from the structurally qualified cold reference; original identity gate retained'
    save()
except BaseException as error:
    state.update(phase='failed',error=type(error).__name__+': '+str(error),finished_unix=time.time());save();raise
