"""Actual public natural-text prefill/replay with unchanged exact reference prompts and outputs."""
import ast
import hashlib
import json
import time
from pathlib import Path
from xolotl_fullwindow_long_fixture_v54 import strict_object, refuse_constant

ROOT = Path(__file__).resolve().parent
plan = json.loads((ROOT / 'request-plan.json').read_text())
raw = (ROOT / 'request-only-source.py').read_bytes()
if hashlib.sha256(raw).hexdigest() != plan['request_source_sha256']:
    raise RuntimeError('exact qualified SSE request source differs')
tree = ast.parse(raw)
if sum(isinstance(node, ast.Try) for node in tree.body) != 1:
    raise RuntimeError('exact request-only module boundary differs')
namespace = dict(__file__=str(ROOT / 'request-only.py'), __name__='frozen_request_only')
exec(compile(ast.Module(body=[node for node in tree.body if not isinstance(node, ast.Try)], type_ignores=[]),
             'qualified-fullwindow-SSE-request', 'exec'), namespace)
state = namespace['STATE']
save = namespace['save']
ask = namespace['ask']


def records(row, fixture, cached):
    usage = row['usage']
    if (row['finish_reason'] != 'stop' or usage['prompt_tokens'] != fixture['actual_prompt_tokens']
            or usage['completion_tokens'] > 64):
        raise RuntimeError('natural public retrieval did not finish/account completely')
    reused = usage.get('prompt_tokens_details', {}).get('cached_tokens')
    if type(reused) is not int or (reused != 0 if not cached else not fixture['actual_prompt_tokens'] - 8 <= reused <= fixture['actual_prompt_tokens']):
        raise RuntimeError('natural public cold/replay KV accounting differs')
    value = json.loads(row['content'], object_pairs_hook=strict_object, parse_constant=refuse_constant)
    if (type(value) is not dict or value.keys() != fixture['expected'].keys()
            or any(type(value[key]) is not int or value[key] != fixture['expected'][key] for key in value)):
        raise RuntimeError('natural public begin/middle/end retrieval differs')


try:
    state.update(schema='aster-xolotl-natural-prefill-v69', phase='preflight', profile=plan['profile'],
                 scope='Actual8244/25671 public technical-document input; configured1M; not actual1M input.',
                 actual1m_input_tested=False, cases=[])
    save()
    reference = None
    if plan.get('reference_root'):
        reference = json.loads((Path(plan['reference_root']) / 'request-result.json').read_text())
        if reference['phase'] != 'passed' or len(reference['tests']) != 7:
            raise RuntimeError('exact completed GPFS natural-text reference required')
    control = ask('short-control-42', 'Calculate 17 + 25. Reply with only the integer answer.', max_tokens=16)
    if control['content'].strip() != '42':
        raise RuntimeError('natural profile short arithmetic control failed')
    fixtures = plan['fixtures']
    if [row['actual_prompt_tokens'] for row in fixtures] != [8244, 25671]:
        raise RuntimeError('same exact two natural input lengths required')
    for index, fixture in enumerate(fixtures):
        path = Path(plan['fixture_root']) / fixture['file']
        raw = path.read_bytes()
        if len(raw) != fixture['prompt_bytes'] or hashlib.sha256(raw).hexdigest() != fixture['prompt_sha256']:
            raise RuntimeError('exact public natural prompt bytes differ')
        text = raw.decode()
        state.update(phase='cold_natural', current_case=fixture['case'])
        save()
        cold = ask('cold-' + fixture['case'], text, max_tokens=64, budget=240)
        records(cold, fixture, False)
        instruction, expected = [('Calculate 19 + 34. Reply with only the integer answer.', '53'),
                                 ('Calculate 19 + 28. Reply with only the integer answer.', '47')][index]
        isolated = ask('isolation-' + expected, instruction, max_tokens=16)
        if isolated['content'].strip() != expected:
            raise RuntimeError('natural-text interleaved isolation failed')
        state['phase'] = 'restoring_natural'
        save()
        restored = ask('restored-' + fixture['case'], text, max_tokens=64)
        records(restored, fixture, True)
        if restored['output_sha256'] != cold['output_sha256']:
            raise RuntimeError('same natural prompt replay changed its complete output')
        if reference:
            wanted = next(row for row in reference['tests'] if row['name'] == cold['name'])
            if cold['output_sha256'] != wanted['output_sha256']:
                raise RuntimeError('Engram directory control changed the matched natural retrieval output')
        state['cases'].append(dict(case=fixture['case'], actual_input_tokens=fixture['actual_prompt_tokens'],
            prompt_sha256=fixture['prompt_sha256'], cold_ttft_seconds=cold['ttft_seconds'],
            cold_seconds=cold['seconds'], replay_ttft_seconds=restored['ttft_seconds'],
            replay_seconds=restored['seconds'], output_sha256=cold['output_sha256'],
            exact_output_reference_matched=reference is not None))
        save()
    state.update(phase='passed', handoff_three_position_retrieval_passed=True,
                 natural_fixture_reference_matched=reference is not None, finished_unix=time.time())
    save()
except BaseException as error:
    state.update(phase='failed', error=type(error).__name__ + ': ' + str(error), finished_unix=time.time())
    save()
    raise
