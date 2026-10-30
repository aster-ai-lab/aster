"""Bind selected native question/form runs without replacing earlier failures."""
import hashlib
import json
from pathlib import Path
import re
import subprocess

CAMPAIGN = Path(__file__).resolve().parent.parent
ROOT = CAMPAIGN.parents[3]
LAYOUT = '2484faae030dbb8ee7bd1c765db3ca28ad1988bb'
ERROR = 'd027629c78e5b0923727fadaae5ba3d8bd41c742'


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def host_path(value):
    if ROOT.drive and value.startswith('/mnt/d/'):
        value = 'D:/' + value[len('/mnt/d/'):]
    return Path(value)


def binding(path):
    return {'file': path.relative_to(CAMPAIGN).as_posix(), 'sha256': sha(path)}


def source_digest(overrides):
    paths = subprocess.check_output([
        'git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard', '--',
        'apps/aster', 'crates/code', 'crates/terminal', 'crates/third-party/crossterm',
        'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'scripts/cli_pair_build.rs',
    ], cwd=ROOT).decode().split('\0')
    process = subprocess.Popen(['git', 'hash-object', '--stdin'], cwd=ROOT,
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    for name in sorted(set(paths) - {''}):
        path = overrides.get(name, ROOT / name)
        if not path.is_file() and not path.is_symlink():
            continue
        raw = str(path.readlink()).encode() if path.is_symlink() else path.read_bytes()
        process.stdin.write(name.encode() + b'\0' + len(raw).to_bytes(8, 'little') + raw)
    process.stdin.close()
    digest = process.stdout.read().decode().strip()
    assert process.wait() == 0
    return digest


snapshot_path = CAMPAIGN / 'source/forms-20261030-error-source.json'
snapshot = json.loads(snapshot_path.read_text())
patch = snapshot_path.parent / snapshot['patch']
assert patch.stat().st_size == snapshot['bytes'] and sha(patch) == snapshot['sha256']
for item in snapshot['files']:
    path = ROOT / item['path']
    assert path.stat().st_size == item['bytes'] and sha(path) == item['sha256'], item['path']
assert len(snapshot['files']) == 55
subprocess.run(['git', 'apply', '--reverse', '--check', str(patch)], cwd=ROOT, check=True)
assert source_digest({}) == ERROR
archive_path = CAMPAIGN / 'source/forms-2484-reconstructed-source.json'
archive = json.loads(archive_path.read_text())
overrides = {}
for item in archive['files']:
    path = CAMPAIGN / item['archive']
    assert path.stat().st_size == item['bytes'] and sha(path) == item['sha256'], item['archive']
    overrides[item['path']] = path
assert len(overrides) == 55 and source_digest(overrides) == LAYOUT

groups = [
    ('questions-20261030-initial', 'e1a37d311f896ceebb6a7a228a0f6a889f15313b',
     ['form-schema-refusals']),
    ('forms-20261030-layout-qualified-pair/reports', LAYOUT,
     ['question-choice-free-text', 'question-cancel-protocol-recovery',
      'form-catalog-native-profile', 'form-cancel-continue']),
    ('forms-20261030-error-qualified-pair/reports', ERROR,
     ['form-constraint-edits', 'form-long-small-window']),
]
platforms = []
inventories = {}
for platform in ('windows', 'linux'):
    reports = []
    pairs = []
    for phase, digest, cases in groups:
        for case in cases:
            path = CAMPAIGN / platform / phase / (case + '.json')
            report = json.loads(path.read_text())
            assert report['assertions_finished'] and not report['panicked'], path
            assert report['config_unchanged'] and report['model'] == 'kimi/k3.1-pro'
            pair = next(o['detail'] for o in report['oracles'] if o.get('name') == 'source-pair')
            assert pair.split(':')[1] == digest
            receipts = []
            for fact in report['facts']:
                if fact['type'] != 'tool.result':
                    continue
                try:
                    outer = json.loads(fact['payload']['output'])
                    inner = json.loads(outer.get('text', '{}'))
                except (ValueError, TypeError):
                    continue
                if inner.get('schema') == 'a3s.code.user-form.v1':
                    receipts.append(inner)
            expected = {'form-catalog-native-profile': 8, 'form-long-small-window': 24,
                        'form-constraint-edits': 3}.get(case)
            if expected:
                answered = [r for r in receipts if r.get('status') == 'answered']
                assert len(answered) == 1 and len(answered[0]['values']) == expected
            if case == 'form-long-small-window':
                assert answered[0]['values']['field24'] == 'LAST-ANSWER-中文'
                for n in range(1, 24):
                    assert answered[0]['values'][f'field{n:02}'] == f'seed{n:02}'
                names = {o.get('name') for o in report['oracles']}
                assert all(f'small-window-field-{n:02}' in names for n in range(1, 25))
            if case == 'form-catalog-native-profile':
                assert answered[0]['values'] == {
                    'name': '用户甲', 'count': 4.0, 'email': 'host@example.invalid',
                    'phone': '020-42', 'notes': '中文备注', 'choice': 'beta',
                    'level': 6.0, 'enabled': True,
                }
            reports.append({**binding(path), 'case': case, 'source_pair': pair,
                            'answered_field_count': expected})
    for phase, digest in [('forms-20261030-layout-qualified-pair', LAYOUT),
                          ('forms-20261030-error-qualified-pair', ERROR)]:
        audit = CAMPAIGN / platform / phase
        manifest_path = host_path((audit / 'manifest-path.txt').read_text().strip())
        pair = json.loads(manifest_path.read_text())
        harness_path = audit / 'test-binary.json'
        harness = json.loads(harness_path.read_text())
        assert pair['source_pair'] == harness['source_pair']
        assert pair['source_pair'].split(':')[1] == digest
        assert harness['entrypoint_count'] == len(set(harness['entrypoints'])) == 67
        inventories[(platform, phase)] = set(harness['entrypoints'])
        binary = host_path(harness['binary'])
        assert sha(binary) == harness['sha256'] and binary.stat().st_size == harness['bytes']
        magic = b'MZ' if platform == 'windows' else b'\x7fELF'
        for item in pair['files']:
            path = host_path(pair['folder']) / item['name']
            assert path.stat().st_size == item['bytes'] and sha(path) == item['sha256']
            with path.open('rb') as stream:
                assert stream.read(len(magic)) == magic
        assert len(pair['files']) == 2
        pairs.append({'source_pair': pair['source_pair'], 'binaries': pair['files'],
                      'harness': binding(harness_path), 'inventory_count': 67})
    gates = []
    for phase, name, passed, ignored in [
        ('questions-20261030-cancel-qualified-pair', 'acp-library.log',
         363 if platform == 'windows' else 398, 8),
        ('forms-20261030-layout-qualified-pair', 'elicitation-error-library-native.log', 25, 0),
    ]:
        path = CAMPAIGN / platform / phase / name
        text = path.read_text(encoding='utf-8-sig')
        summaries = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
        assert summaries == [(str(passed), '0', str(ignored))], path
        gates.append({**binding(path), 'passed': passed, 'ignored': ignored})
    platforms.append({'platform': platform, 'selected_passed_reports': reports,
                      'native_pairs': pairs, 'library_gates': gates})
assert len({frozenset(names) for names in inventories.values()}) == 1
result = {
    'schema': 'aster.tui.questions-forms-runs.v1', 'model': 'kimi/k3.1-pro',
    'complete_153': False, 'whole_pager_qualified': False,
    'all_67_entrypoints_run_together': False, 'original_failures_retained': True,
    'selected_live_cases_per_platform': 7,
    'current_source_snapshot': binding(snapshot_path), 'current_source_files': 55,
    'reconstructed_source_archive': binding(archive_path),
    'platforms': platforms,
}
destination = CAMPAIGN / 'RUNS-QUESTIONS-FORMS-20261030.json'
assert not destination.exists(), 'retain the previous reconciliation'
destination.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
assert json.loads(destination.read_text(encoding='utf-8')) == result
print(json.dumps({'selected_cases_per_platform': 7, 'source_files': 55,
                  'native_pairs': 4, 'complete_153': False}))
