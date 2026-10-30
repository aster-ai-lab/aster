"""Reconcile the frozen 13-case batch without dropping failed evaluations."""
import hashlib
import argparse
import json
from pathlib import Path
import re

CAMPAIGN = Path(__file__).resolve().parent.parent
ROOT = CAMPAIGN.parents[3]
parser = argparse.ArgumentParser()
parser.add_argument('--ui-qualified', action='store_true')
args = parser.parse_args()
PHASE = 'coverage-20261030-ui-qualified' if args.ui_qualified else 'coverage-20261030-portable'
CASE_COUNT = 3 if args.ui_qualified else 13
INVENTORY = 99 if args.ui_qualified else 96


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def ref(path):
    return {'file': path.relative_to(CAMPAIGN).as_posix(), 'sha256': sha(path)}


def local(value):
    return Path('D:/' + value[7:] if ROOT.drive and value.startswith('/mnt/d/') else value)


snapshot_path = CAMPAIGN / ('source/coverage-20261030-ui-qualified-source.json' if args.ui_qualified else 'source/coverage-20261030-source.json')
snapshot = json.loads(snapshot_path.read_text(encoding='utf-8'))
original = json.loads((CAMPAIGN / 'source/tool-wave-20261030-source.json').read_text(encoding='utf-8'))
original_files = {item['path']: item['sha256'] for item in original['files']}
assert snapshot['runtime_product_pair_digest'] == original['source_pair_digest']
assert not snapshot['runtime_product_pair_rebuilt']
assert len(snapshot['files']) == (59 if args.ui_qualified else 58)
assert len(snapshot['new_qualified_cases']) == CASE_COUNT
for item in snapshot['files']:
    path = CAMPAIGN / item['archive']
    assert sha(path) == item['sha256'] and path.stat().st_size == item['bytes']
    if original_files.get(item['path']) != item['sha256']:
        assert item['path'].startswith('crates/code/acp/tests/')

phases = []
inventories = []
for platform in ('windows', 'linux'):
    folder = CAMPAIGN / platform / PHASE
    manifest_path = local((folder / 'manifest-path.txt').read_text().strip())
    pair = json.loads(manifest_path.read_text())
    harness_path = folder / 'test-binary.json'
    provenance_path = folder / 'harness-source.json'
    harness = json.loads(harness_path.read_text())
    provenance = json.loads(provenance_path.read_text())
    assert pair['source_pair'] == harness['source_pair'] == provenance['runtime_pair']
    assert pair['source_pair'].split(':')[1] == snapshot['runtime_product_pair_digest']
    assert not provenance['runtime_pair_rebuilt']
    assert provenance['harness_source_digest'] == snapshot['source_digest']
    assert sha(manifest_path) == provenance['original_pair_manifest_sha256']
    assert sha(local(harness['binary'])) == harness['sha256'] == provenance['test_binary_sha256']
    assert harness['entrypoint_count'] == len(set(harness['entrypoints'])) == INVENTORY
    inventories.append(set(harness['entrypoints']))
    for item in pair['files']:
        binary = local(pair['folder']) / item['name']
        assert sha(binary) == item['sha256'] and binary.stat().st_size == item['bytes']
        with binary.open('rb') as stream:
            magic = stream.read(4)
        assert magic.startswith(b'MZ') if platform == 'windows' else magic == b'\x7fELF'
    results_path = folder / 'feature-results.json'
    results = json.loads(results_path.read_text())
    assert [item['test'] for item in results['tests']] == snapshot['new_qualified_cases']
    assert results['harness_source_digest'] == snapshot['source_digest']
    logs = []
    for item in results['tests']:
        log = CAMPAIGN / item['log']
        summary = re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed;.*?finished in ([\d.]+)s', log.read_text(encoding='utf-8-sig'))
        assert len(summary) == 1
        status, passed, failed, seconds = summary[0]
        assert int(passed) + int(failed) == 1
        assert (item['exit_code'] == 0) == (status == 'ok')
        logs.append({**ref(log), **item, 'seconds': float(seconds)})
    reports = []
    for path in sorted((folder / 'reports').glob('*.json')):
        report = json.loads(path.read_text(encoding='utf-8'))
        assert report['model'] == 'kimi/k3.1-pro' and report['config_unchanged']
        assert report['assertions_finished'] != report['panicked']
        assert next(o['detail'] for o in report['oracles'] if o.get('name') == 'source-pair') == pair['source_pair']
        reports.append({**ref(path), 'case': report['case'], 'assertions_finished': report['assertions_finished'], 'panicked': report['panicked']})
    assert len(reports) == CASE_COUNT
    passed = sum(item['exit_code'] == 0 for item in results['tests'])
    assert passed == sum(item['assertions_finished'] for item in reports)
    phases.append({'platform': platform, 'phase': PHASE, 'runtime_pair': pair['source_pair'],
                   'harness': ref(harness_path), 'harness_source': ref(provenance_path),
                   'inventory': INVENTORY, 'passed': passed, 'failed': CASE_COUNT - passed,
                   'seconds': round(sum(item['seconds'] for item in logs), 2),
                   'run_logs': logs, 'case_reports': reports})
assert inventories[0] == inventories[1]
result = {'schema': 'aster.tui.coverage-wave-runs.v1', 'model': 'kimi/k3.1-pro',
          'source_snapshot': ref(snapshot_path), 'runtime_pair_rebuilt': False,
          'pre_execution_harness_failure': ref(CAMPAIGN / 'source/coverage-20261030-pre-execution-failure.json'),
          'phases': phases, 'inventory': INVENTORY, 'all_inventory_entrypoints_run': False, 'complete_153': False}
if args.ui_qualified:
    result['original_failed_runs'] = ref(CAMPAIGN / 'RUNS-COVERAGE-20261030.json')
    assert snapshot['original_13_cases_unchanged']
destination = CAMPAIGN / ('RUNS-COVERAGE-UI-QUALIFIED-20261030.json' if args.ui_qualified else 'RUNS-COVERAGE-20261030.json')
assert not destination.exists(), 'retain earlier reconciliation'
destination.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
assert json.loads(destination.read_text(encoding='utf-8')) == result
print(json.dumps({p['platform']: {key: p[key] for key in ('passed', 'failed', 'seconds')} for p in phases}))
