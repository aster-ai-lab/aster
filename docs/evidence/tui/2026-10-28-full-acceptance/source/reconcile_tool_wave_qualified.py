"""Audit revised harnesses separately from their reused, unchanged product pair."""
import hashlib
import json
from pathlib import Path
import re

CAMPAIGN = Path(__file__).resolve().parent.parent
ROOT = CAMPAIGN.parents[3]
RUNTIME = 'c860e66c1e42149f47a9bfadff219fd6fae2f8fa'


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def ref(path):
    return {'file': path.relative_to(CAMPAIGN).as_posix(), 'sha256': sha(path)}


def local(value):
    return Path('D:/' + value[7:] if ROOT.drive and value.startswith('/mnt/d/') else value)


original_snapshot = json.loads((CAMPAIGN / 'source/tool-wave-20261030-source.json').read_text(encoding='utf-8'))
original_files = {i['path']: i for i in original_snapshot['files']}
snapshots = {}
for name in ('tool-wave-20261030-qualified-source.json', 'tool-wave-20261030-checklist-exit-source.json'):
    path = CAMPAIGN / 'source' / name
    data = json.loads(path.read_text(encoding='utf-8'))
    assert data['runtime_product_pair_digest'] == RUNTIME and not data['runtime_product_pair_rebuilt']
    assert len(data['files']) == 57
    for item in data['files']:
        archived = CAMPAIGN / item['archive']
        assert archived.stat().st_size == item['bytes'] and sha(archived) == item['sha256']
        if item['sha256'] != original_files[item['path']]['sha256']:
            assert item['path'].startswith('crates/code/acp/tests/')
    snapshots[path.relative_to(CAMPAIGN).as_posix()] = {**ref(path), 'source_digest': data['source_digest']}

phases = []
reports_by_key = {}
inventories = []
for platform, phase_name, count, cases in [
    ('windows', 'tool-wave-20261030-qualified', 82, 5),
    ('linux', 'tool-wave-20261030-qualified', 82, 5),
    ('windows', 'tool-wave-20261030-checklist-exit', 83, 1),
]:
    phase = CAMPAIGN / platform / phase_name
    pair = json.loads(local((phase / 'manifest-path.txt').read_text().strip()).read_text())
    harness_path = phase / 'test-binary.json'
    harness = json.loads(harness_path.read_text())
    provenance_path = phase / 'harness-source.json'
    provenance = json.loads(provenance_path.read_text())
    assert pair['source_pair'] == harness['source_pair'] == provenance['runtime_pair']
    assert pair['source_pair'].split(':')[1] == RUNTIME
    assert not provenance['runtime_pair_rebuilt']
    assert provenance['harness_source_digest'] == snapshots[provenance['harness_snapshot']]['source_digest']
    assert harness['entrypoint_count'] == len(set(harness['entrypoints'])) == count
    assert sha(local(harness['binary'])) == harness['sha256'] == provenance['test_binary_sha256']
    if count == 82:
        inventories.append(set(harness['entrypoints']))
    for item in pair['files']:
        binary = local(pair['folder']) / item['name']
        assert binary.stat().st_size == item['bytes'] and sha(binary) == item['sha256']
    results_path = phase / 'feature-results.json'
    results = json.loads(results_path.read_text())
    assert len(results['tests']) == cases
    logs = []
    for item in results['tests']:
        path = CAMPAIGN / item['log']
        summary = re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed;.*?finished in ([\d.]+)s', path.read_text(encoding='utf-8-sig'))
        assert len(summary) == 1
        status, passed, failed, seconds = summary[0]
        assert int(passed) + int(failed) == 1 and (item['exit_code'] == 0) == (status == 'ok')
        logs.append({**ref(path), **item, 'seconds': float(seconds)})
    reports = []
    for path in sorted((phase / 'reports').glob('*.json')):
        data = json.loads(path.read_text())
        assert data['model'] == 'kimi/k3.1-pro' and data['config_unchanged']
        assert data['assertions_finished'] != data['panicked']
        assert next(o['detail'] for o in data['oracles'] if o.get('name') == 'source-pair') == pair['source_pair']
        reports_by_key[(platform, phase_name, data['case'])] = data
        reports.append({**ref(path), 'case': data['case'], 'assertions_finished': data['assertions_finished'], 'panicked': data['panicked']})
    assert len(reports) == cases
    passed = sum(i['exit_code'] == 0 for i in results['tests'])
    assert passed == sum(i['assertions_finished'] for i in reports)
    phases.append({'platform': platform, 'phase': phase_name, 'runtime_pair': pair['source_pair'],
                   'harness': ref(harness_path), 'harness_source': ref(provenance_path),
                   'inventory': count, 'passed': passed, 'failed': cases - passed,
                   'run_logs': logs, 'case_reports': reports})
assert inventories[0] == inventories[1]

for platform, phase, case, glyphs in [
    ('windows', 'tool-wave-20261030-checklist-exit', 'tool-wave-checklist-exit', ('√', 'x')),
    ('linux', 'tool-wave-20261030-qualified', 'tool-wave-checklist-traced', ('✓', '✗')),
]:
    data = reports_by_key[(platform, phase, case)]
    assert data['assertions_finished']
    screen = next(o['screen'] for o in data['oracles'] if o.get('name') == 'checklist-replacement-ui')
    assert glyphs[0] + ' CHECKLIST-SECOND' in screen
    assert glyphs[1] + ' CHECKLIST-FIRST' in screen
    assert 'CHECKLIST-REMOVED' not in screen
for platform in ('windows', 'linux'):
    data = reports_by_key[(platform, 'tool-wave-20261030-qualified', 'tool-wave-task-children-traced')]
    assert data['assertions_finished']
    children = [json.loads(o['detail']) for o in data['oracles'] if o.get('name') == 'child-owned-wire-lifecycle']
    assert len(children) == 2 and children[0]['task_id'] != children[1]['task_id']
    assert children[0]['session_id'] != children[1]['session_id']
    assert children[0]['parent_session_id'] == children[1]['parent_session_id']
    for child in children:
        assert child['start']['subagent_id'] == child['end']['subagent_id'] == child['task_id']
        assert child['start']['child_session_id'] == child['end']['child_session_id'] == child['session_id']
        assert child['end']['status'] == 'completed' and child['end']['tool_calls'] >= 1
        assert child['progress_packets'] >= 1
result = {'schema': 'aster.tui.tool-wave-qualified-runs.v1', 'model': 'kimi/k3.1-pro',
          'runtime_pair_digest': RUNTIME, 'runtime_pair_rebuilt': False,
          'original_failed_runs': ref(CAMPAIGN / 'RUNS-TOOL-WAVE-20261030.json'),
          'harness_snapshots': list(snapshots.values()), 'phases': phases,
          'all_82_or_83_entrypoints_run': False, 'original_failures_retained': True, 'complete_153': False}
destination = CAMPAIGN / 'RUNS-TOOL-WAVE-QUALIFIED-20261030.json'
assert not destination.exists(), 'retain earlier reconciliation'
destination.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
assert json.loads(destination.read_text(encoding='utf-8')) == result
print(json.dumps({p['platform'] + '/' + p['phase']: {'passed': p['passed'], 'failed': p['failed']} for p in phases}))
