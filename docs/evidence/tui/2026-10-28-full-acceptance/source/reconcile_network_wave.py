"""Retain transport setup failures and independently qualify native HTTP cases."""
import hashlib
import json
from pathlib import Path
import re

C = Path(__file__).resolve().parent.parent
ROOT = C.parents[3]
RUNTIME = 'c860e66c1e42149f47a9bfadff219fd6fae2f8fa'


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def ref(path):
    return {'file': path.relative_to(C).as_posix(), 'sha256': sha(path)}


def local(value):
    return Path('D:/' + value[7:] if ROOT.drive and value.startswith('/mnt/d/') else value)


snapshots = {}
original = json.loads((C / 'source/tool-wave-20261030-source.json').read_text())
product_sources = {item['path']: item for item in original['files']}
for label in ('network-20261030', 'network-20261030-provider-direct', 'network-20261030-pagination', 'network-20261030-direct-pages'):
    path = C / 'source' / (label + '-source.json')
    data = json.loads(path.read_text())
    assert data['runtime_product_pair_digest'] == RUNTIME and not data['runtime_product_pair_rebuilt']
    assert len(data['files']) == 60
    for item in data['files']:
        archive = C / item['archive']
        assert sha(archive) == item['sha256'] and archive.stat().st_size == item['bytes']
        if product_sources.get(item['path'], {}).get('sha256') != item['sha256']:
            assert item['path'].startswith('crates/code/acp/tests/')
    snapshots[label] = data
net = 'crates/code/acp/tests/aster_tui_live_e2e/network_tools_wave.rs'
initial_file = next(item for item in snapshots['network-20261030']['files'] if item['path'] == net)
direct_file = next(item for item in snapshots['network-20261030-provider-direct']['files'] if item['path'] == net)
page_file = next(item for item in snapshots['network-20261030-pagination']['files'] if item['path'] == net)
assert initial_file['sha256'] == direct_file['sha256']
assert (C / page_file['archive']).read_bytes().startswith((C / direct_file['archive']).read_bytes())

phases = []
inventories = {}
reports = {}
for label, phase_name, expected_count, provider_direct in [
    ('network-20261030', 'network-20261030-native', 14, False),
    ('network-20261030-provider-direct', 'network-20261030-provider-direct', 14, True),
    ('network-20261030-pagination', 'network-20261030-pagination', 1, True),
    ('network-20261030-direct-pages', 'network-20261030-direct-pages', 1, True),
]:
    snapshot = snapshots[label]
    inventory = snapshot['expected_inventory']
    for platform in ('windows', 'linux'):
        phase = C / platform / phase_name
        manifest_path = local((phase / 'manifest-path.txt').read_text().strip())
        pair = json.loads(manifest_path.read_text())
        harness_path = phase / 'test-binary.json'
        provenance_path = phase / 'harness-source.json'
        harness = json.loads(harness_path.read_text())
        provenance = json.loads(provenance_path.read_text())
        assert pair['source_pair'] == harness['source_pair'] == provenance['runtime_pair']
        assert pair['source_pair'].split(':')[1] == RUNTIME
        assert not provenance['runtime_pair_rebuilt']
        assert provenance.get('provider_direct_fixture_profile', False) == provider_direct
        assert provenance['harness_source_digest'] == snapshot['source_digest']
        assert sha(manifest_path) == provenance['original_pair_manifest_sha256']
        assert sha(local(harness['binary'])) == harness['sha256'] == provenance['test_binary_sha256']
        assert harness['entrypoint_count'] == len(set(harness['entrypoints'])) == inventory
        inventories[(phase_name, platform)] = set(harness['entrypoints'])
        for item in pair['files']:
            binary = local(pair['folder']) / item['name']
            assert sha(binary) == item['sha256'] and binary.stat().st_size == item['bytes']
            with binary.open('rb') as stream:
                magic = stream.read(4)
            assert magic.startswith(b'MZ') if platform == 'windows' else magic == b'\x7fELF'
        results_path = phase / 'feature-results.json'
        results = json.loads(results_path.read_text())
        assert [item['test'] for item in results['tests']] == snapshot['new_qualified_cases']
        assert len(results['tests']) == expected_count
        logs = []
        for item in results['tests']:
            path = C / item['log']
            summaries = re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed;.*?finished in ([\d.]+)s', path.read_text(encoding='utf-8-sig'))
            assert len(summaries) == 1
            status, passed, failed, seconds = summaries[0]
            assert int(passed) + int(failed) == 1
            assert (item['exit_code'] == 0) == (status == 'ok')
            logs.append({**ref(path), **item, 'seconds': float(seconds)})
        case_reports = []
        for path in sorted((phase / 'reports').glob('*.json')):
            report = json.loads(path.read_text())
            assert report['model'] == 'kimi/k3.1-pro' and report['config_unchanged']
            assert report['assertions_finished'] != report['panicked']
            assert next(o['detail'] for o in report['oracles'] if o.get('name') == 'source-pair') == pair['source_pair']
            if not provider_direct:
                assert not report['assertions_finished']
                assert not any(f['type'] == 'tool.prepared' for f in report['facts'])
            reports[(platform, phase_name, report['case'])] = report
            case_reports.append({**ref(path), 'case': report['case'], 'assertions_finished': report['assertions_finished'], 'panicked': report['panicked']})
        assert len(case_reports) == expected_count
        passed = sum(item['exit_code'] == 0 for item in results['tests'])
        assert passed == sum(item['assertions_finished'] for item in case_reports)
        phases.append({'platform': platform, 'phase': phase_name, 'runtime_pair': pair['source_pair'],
                       'source_snapshot': ref(C / 'source' / (label + '-source.json')),
                       'harness': ref(harness_path), 'harness_source': ref(provenance_path),
                       'inventory': inventory, 'provider_direct_fixture_profile': provider_direct,
                       'passed': passed, 'failed': expected_count - passed,
                       'seconds': round(sum(item['seconds'] for item in logs), 2),
                       'run_logs': logs, 'case_reports': case_reports})
    assert inventories[(phase_name, 'windows')] == inventories[(phase_name, 'linux')]

win = reports[('windows', 'network-20261030-provider-direct', 'network-download-binary')]
assert win['panicked']
download = [json.loads(f['payload']['output']) for f in win['facts'] if f['type'] == 'tool.result']
download = next(item for item in download if item.get('metadata', {}).get('file_path') == 'artifacts/下载 payload.bin')
assert download['exit_code'] == 0 and download['metadata']['bytes'] == 65536
assert download['metadata']['sha256'] == '4b640d85ab3ba30fd02c9fc9db4a8928f416322ad27022ea58a65aaee68a4df2'
assert not any(f['type'] == 'turn.complete' for f in win['facts'])
observation = C / 'source/network-20261030-windows-pending-shell-observation.json'
assert json.loads(observation.read_text())['matches_exact_fixture_bytes']
correction = C / 'source/network-20261030-process-identity-correction.json'
assert not json.loads(correction.read_text())['process_modified_or_terminated']
artifact_denial = reports[('windows', 'network-20261030-pagination', 'network-fetch-pages-inline-read')]
assert artifact_denial['panicked']
denials = [f['payload'] for f in artifact_denial['facts'] if f['type'] == 'tool.result' and f['payload'].get('outcome') == 'denied']
assert len(denials) == 1
assert denials[0]['output'].startswith('tool_effect_effective_arguments_unapproved:')
for platform in ('windows', 'linux'):
    assert reports[(platform, 'network-20261030-direct-pages', 'network-fetch-pages-direct')]['assertions_finished']
    for case in ('network-fetch-main', 'network-fetch-redirect'):
        report = reports[(platform, 'network-20261030-provider-direct', case)]
        assert report['assertions_finished']
        assert any('aster-fixture.example.test' in (oracle.get('screen') or '') for oracle in report['oracles'])

result = {'schema': 'aster.tui.network-wave-runs.v1', 'model': 'kimi/k3.1-pro',
          'runtime_pair_digest': RUNTIME, 'runtime_pair_rebuilt': False,
          'original_14_case_source_retained': True, 'original_failures_retained': True,
          'windows_independent_file_observation': ref(observation), 'process_identity_correction': ref(correction),
          'phases': phases, 'all_inventory_entrypoints_run': False, 'complete_153': False}
destination = C / 'RUNS-NETWORK-20261030.json'
assert not destination.exists(), 'retain earlier reconciliation'
destination.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
assert json.loads(destination.read_text(encoding='utf-8')) == result
print(json.dumps({p['platform'] + '/' + p['phase']: {k: p[k] for k in ('passed', 'failed', 'seconds')} for p in phases}))
