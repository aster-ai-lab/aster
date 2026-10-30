"""Keep native ownership passes, a model error, and genuine step failures distinct."""
import hashlib
import json
from pathlib import Path
import re

CAMPAIGN = Path(__file__).resolve().parent.parent
ROOT = CAMPAIGN.parents[3]
PHASE = 'forms-20261030-owner-step-initial'
DIGEST = 'b44752d77fa649bf767498820c1fdb123aaa13ac'


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def path_on_host(value):
    return Path('D:/' + value[len('/mnt/d/'):] if ROOT.drive and value.startswith('/mnt/d/') else value)


def reference(path):
    return {'file': path.relative_to(CAMPAIGN).as_posix(), 'sha256': sha(path)}


def report(path, passed):
    data = json.loads(path.read_text(encoding='utf-8'))
    assert data['model'] == 'kimi/k3.1-pro' and data['config_unchanged']
    assert data['assertions_finished'] == passed and data['panicked'] != passed
    pair = next(o['detail'] for o in data['oracles'] if o.get('name') == 'source-pair')
    assert pair.split(':')[1] == DIGEST
    return data, {**reference(path), 'source_pair': pair, 'assertions_finished': passed}


def submitted_forms(data):
    result = []
    for fact in data['facts']:
        if fact['type'] != 'tool.result':
            continue
        try:
            outer = json.loads(fact['payload']['output'])
            inner = json.loads(outer.get('text', '{}'))
        except (ValueError, TypeError):
            continue
        if inner.get('schema') == 'a3s.code.user-form.v1':
            result.append(inner)
    return result


snapshot_path = CAMPAIGN / 'source/forms-20261030-owner-step-initial-source.json'
snapshot = json.loads(snapshot_path.read_text(encoding='utf-8'))
assert snapshot['source_pair_digest'] == DIGEST and len(snapshot['files']) == 56
for item in snapshot['files']:
    for path in (ROOT / item['path'], CAMPAIGN / item['archive']):
        assert path.stat().st_size == item['bytes'] and sha(path) == item['sha256'], path

platforms = []
inventories = []
for platform in ('windows', 'linux'):
    audit = CAMPAIGN / platform / PHASE
    pair = json.loads(path_on_host((audit / 'manifest-path.txt').read_text().strip()).read_text())
    harness_path = audit / 'test-binary.json'
    harness = json.loads(harness_path.read_text())
    assert pair['source_pair'] == harness['source_pair']
    assert pair['source_pair'].split(':')[1] == DIGEST
    assert harness['entrypoint_count'] == len(set(harness['entrypoints'])) == 69
    inventories.append(set(harness['entrypoints']))
    executable = path_on_host(harness['binary'])
    assert sha(executable) == harness['sha256']
    magic = b'MZ' if platform == 'windows' else b'\x7fELF'
    for item in pair['files']:
        binary = path_on_host(pair['folder']) / item['name']
        assert binary.stat().st_size == item['bytes'] and sha(binary) == item['sha256']
        with binary.open('rb') as stream:
            assert stream.read(len(magic)) == magic
    assert len(pair['files']) == 2
    owner, owner_ref = report(audit / 'reports/form-session-owner.json', True)
    checkpoints = {o['checkpoint']: o for o in owner['oracles'] if 'checkpoint' in o}
    a = checkpoints['form-session-owner-a']
    b = checkpoints['form-session-owner-b']
    assert a['session'] != b['session']
    forms = submitted_forms({'facts': a['facts']})
    assert len(forms) == 1 and forms[0]['status'] == 'answered'
    answer = forms[0]['values']['value']
    assert answer.startswith('A-ONLY-中文-')
    assert not any(f['type'] == 'tool.prepared' for f in b['facts'])
    assert answer not in json.dumps(b['facts'], ensure_ascii=False)
    names = {o.get('name') for o in owner['oracles']}
    assert {'background-form-does-not-capture-b', 'repeated-submit-has-one-owned-receipt',
            'other-session-after-submit'}.issubset(names)
    step_phase = 'forms-20261030-step-exact-reprobe' if platform == 'windows' else PHASE
    step, step_ref = report(CAMPAIGN / platform / step_phase / 'reports/form-step-constraints.json', False)
    forms = submitted_forms(step)
    assert len(forms) == 1 and forms[0]['values'] == {'level': 3.0, 'count': 2.0}
    step_ref['classification'] = 'product defect: step=2 accepts and returns level=3'
    step_ref['actual_values'] = forms[0]['values']
    library = audit / 'owner-library.log'
    assert re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',
                      library.read_text(encoding='utf-8-sig')) == [('1', '0', '0')]
    platforms.append({'platform': platform, 'source_pair': pair['source_pair'],
                      'binaries': pair['files'], 'harness': reference(harness_path),
                      'library_owner_test': reference(library), 'native_owner_case': owner_ref,
                      'native_step_failure': step_ref})
assert inventories[0] == inventories[1]
initial, initial_ref = report(CAMPAIGN / 'windows' / PHASE / 'reports/form-step-constraints.json', False)
call = next(f['payload']['call'] for f in initial['facts']
            if f['type'] == 'model.turn' and f['payload']['call'].get('name') == 'ask_user_form')
try:
    json.loads(call['args']['form'])
except json.JSONDecodeError:
    initial_ref['classification'] = 'model transcription error: invalid JSON document; no native form was opened'
else:
    raise AssertionError('initial Windows failure needs reclassification')
assert not submitted_forms(initial)
result = {'schema': 'aster.tui.form-owner-step-runs.v1', 'model': 'kimi/k3.1-pro',
          'source_pair_digest': DIGEST, 'source_snapshot': reference(snapshot_path),
          'same_native_69_entrypoint_inventory': True, 'all_69_entrypoints_run': False,
          'platforms': platforms, 'excluded_model_evaluation': initial_ref,
          'step_qualified': False, 'complete_153': False}
destination = CAMPAIGN / 'RUNS-FORMS-OWNER-STEP-20261030.json'
assert not destination.exists(), 'retain earlier reconciliation'
destination.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
assert json.loads(destination.read_text(encoding='utf-8')) == result
print(json.dumps({'native_owner_passes': 2, 'native_step_product_failures': 2,
                  'retained_model_errors': 1, 'source_files': 56, 'complete_153': False}))
