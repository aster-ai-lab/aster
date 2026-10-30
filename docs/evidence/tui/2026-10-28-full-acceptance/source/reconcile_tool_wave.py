"""Bind every result of the original eight-case tool wave, including failures."""
import hashlib
import json
from pathlib import Path
import re

CAMPAIGN = Path(__file__).resolve().parent.parent
ROOT = CAMPAIGN.parents[3]
DIGEST = 'c860e66c1e42149f47a9bfadff219fd6fae2f8fa'
PHASE = 'tool-wave-20261030-native'


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def host_path(value):
    return Path('D:/' + value[7:] if ROOT.drive and value.startswith('/mnt/d/') else value)


def ref(path):
    return {'file': path.relative_to(CAMPAIGN).as_posix(), 'sha256': sha(path)}


snapshot_path = CAMPAIGN / 'source/tool-wave-20261030-source.json'
snapshot = json.loads(snapshot_path.read_text(encoding='utf-8'))
assert snapshot['source_pair_digest'] == DIGEST and len(snapshot['files']) == 57
for item in snapshot['files']:
    path = CAMPAIGN / item['archive']
    assert path.stat().st_size == item['bytes'] and sha(path) == item['sha256']
expected = {'tool_wave::' + name for name in snapshot['new_live_cases']}
platforms = []
inventories = []
for platform in ('windows', 'linux'):
    phase = CAMPAIGN / platform / PHASE
    pair = json.loads(host_path((phase / 'manifest-path.txt').read_text().strip()).read_text())
    harness_path = phase / 'test-binary.json'
    harness = json.loads(harness_path.read_text())
    assert pair['source_pair'] == harness['source_pair']
    assert pair['source_pair'].split(':')[1] == DIGEST
    assert len(set(harness['entrypoints'])) == harness['entrypoint_count'] == 77
    inventories.append(set(harness['entrypoints']))
    assert sha(host_path(harness['binary'])) == harness['sha256']
    for item in pair['files']:
        binary = host_path(pair['folder']) / item['name']
        assert binary.stat().st_size == item['bytes'] and sha(binary) == item['sha256']
        with binary.open('rb') as stream:
            assert stream.read(4).startswith(b'MZ' if platform == 'windows' else b'\x7fELF')
    assert len(pair['files']) == 2
    results_path = phase / 'feature-results.json'
    results = json.loads(results_path.read_text())
    assert results['source_pair'] == pair['source_pair']
    assert {r['test'] for r in results['tests']} == expected and len(results['tests']) == 8
    logs = []
    for item in results['tests']:
        path = CAMPAIGN / item['log']
        summary = re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed;.*?finished in ([\d.]+)s', path.read_text(encoding='utf-8-sig'))
        assert len(summary) == 1
        status, passed, failed, seconds = summary[0]
        assert int(passed) + int(failed) == 1
        assert (item['exit_code'] == 0) == (status == 'ok')
        logs.append({**ref(path), **item, 'seconds': float(seconds)})
    reports = []
    for path in sorted((phase / 'reports').glob('*.json')):
        report = json.loads(path.read_text())
        assert report['model'] == 'kimi/k3.1-pro' and report['config_unchanged']
        assert report['assertions_finished'] != report['panicked']
        identity = next(o['detail'] for o in report['oracles'] if o.get('name') == 'source-pair')
        assert identity == pair['source_pair']
        reports.append({**ref(path), 'case': report['case'],
                        'assertions_finished': report['assertions_finished'], 'panicked': report['panicked']})
    assert len(reports) == 8
    passed = sum(r['exit_code'] == 0 for r in results['tests'])
    assert passed == sum(r['assertions_finished'] for r in reports)
    platforms.append({'platform': platform, 'source_pair': pair['source_pair'],
                      'binaries': pair['files'], 'harness': ref(harness_path),
                      'passed': passed, 'failed': 8 - passed, 'run_logs': logs, 'case_reports': reports})
assert inventories[0] == inventories[1]
result = {'schema': 'aster.tui.tool-wave-runs.v1', 'model': 'kimi/k3.1-pro',
          'source_snapshot': ref(snapshot_path), 'source_pair_digest': DIGEST,
          'all_77_entrypoints_run': False, 'complete_153': False,
          'supplemental_runs_do_not_replace_initial_failures': True, 'platforms': platforms}
destination = CAMPAIGN / 'RUNS-TOOL-WAVE-20261030.json'
assert not destination.exists(), 'retain the previous reconciliation'
destination.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8', newline='\n')
assert json.loads(destination.read_text()) == result
print(json.dumps({p['platform']: {'passed': p['passed'], 'failed': p['failed']} for p in platforms}))
