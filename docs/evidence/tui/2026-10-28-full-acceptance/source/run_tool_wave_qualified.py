"""Build a revised observer and reuse the unchanged frozen native product pair."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser()
parser.add_argument('platform', choices=('windows', 'linux'))
parser.add_argument('--config', type=Path, required=True)
parser.add_argument('--phase', default='tool-wave-20261030-qualified')
parser.add_argument('--snapshot', default='source/tool-wave-20261030-qualified-source.json')
parser.add_argument('--provider-direct', action='store_true')
args = parser.parse_args()
root = Path.cwd().resolve()
campaign = Path(__file__).resolve().parent.parent
source = campaign / args.platform / 'tool-wave-20261030-native'
phase = args.phase
audit = campaign / args.platform / phase
assert not audit.exists(), 'retain earlier evaluation artifacts'
audit.mkdir(parents=True)
manifest_path = Path((source / 'manifest-path.txt').read_text().strip())
pair = json.loads(manifest_path.read_text())
for item in pair['files']:
    binary = Path(pair['folder']) / item['name']
    with binary.open('rb') as stream:
        assert hashlib.file_digest(stream, 'sha256').hexdigest() == item['sha256']
    assert subprocess.check_output([str(binary), '--build-id'], text=True).strip() == pair['source_pair']
(audit / 'manifest-path.txt').write_text(str(manifest_path) + '\n', encoding='utf-8', newline='\n')
suffix = '.exe' if args.platform == 'windows' else ''
compiled = root / '.tmp' / (phase + '-tests' + suffix)
with (audit / 'test-build.log').open('wb') as stream:
    code = subprocess.run(['cargo', 'rustc', '--locked', '-p', 'a3s-code-acp', '--test',
                           'aster_tui_live_e2e', '--', '--emit=link=' + str(compiled)],
                          stdout=stream, stderr=subprocess.STDOUT).returncode
if code:
    raise SystemExit(code)
snapshot_path = campaign / args.snapshot
snapshot = json.loads(snapshot_path.read_text(encoding='utf-8'))
with (audit / 'test-freeze.log').open('wb') as stream:
    subprocess.run([sys.executable, str(campaign / 'source/freeze_native_test.py'),
                    args.platform, phase, str(compiled), '--expected-tests', str(snapshot['expected_inventory'])],
                   stdout=stream, stderr=subprocess.STDOUT, check=True)
harness = json.loads((audit / 'test-binary.json').read_text())
provenance = {'runtime_pair': pair['source_pair'], 'runtime_pair_rebuilt': False,
              'harness_source_digest': snapshot['source_digest'],
              'harness_snapshot': snapshot_path.relative_to(campaign).as_posix(),
              'provider_direct_fixture_profile': args.provider_direct,
              'original_pair_manifest_sha256': hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
              'test_binary_sha256': harness['sha256']}
(audit / 'harness-source.json').write_text(json.dumps(provenance, indent=2) + '\n', encoding='utf-8', newline='\n')
env = os.environ.copy()
if args.provider_direct:
    env['A3S_TUI_LIVE_PROVIDER_DIRECT'] = '1'
env.update({'A3S_CONFIG_FILE': str(args.config.resolve()),
            'A3S_CODE_TUI_BIN': str(Path(pair['folder']) / ('aster' + suffix)),
            'A3S_CODE_ACP_BIN': str(Path(pair['folder']) / ('a3s-code-acp' + suffix)),
            'A3S_CODE_ACP_TRACE_PROXY': str(root / '.tmp' / ('acp-trace-proxy.exe' if suffix else 'acp-trace-proxy-linux')),
            'A3S_TUI_LIVE_EVIDENCE_DIR': str(audit / 'reports')})
results = []
for name in snapshot['new_qualified_cases']:
    test = name if '::' in name else 'tool_wave::' + name
    assert test in harness['entrypoints']
    log = audit / (name.replace('::', '__') + '.log')
    with log.open('wb') as stream:
        code = subprocess.run([harness['binary'], test, '--ignored', '--exact', '--nocapture', '--test-threads=1'],
                              env=env, stdout=stream, stderr=subprocess.STDOUT).returncode
    results.append({'test': test, 'exit_code': code, 'log': log.relative_to(campaign).as_posix()})
    (audit / 'feature-results.json').write_text(json.dumps({'source_pair': pair['source_pair'],
        'harness_source_digest': snapshot['source_digest'], 'tests': results}, indent=2) + '\n', encoding='utf-8', newline='\n')
    print(json.dumps(results[-1]), flush=True)
raise SystemExit(int(any(r['exit_code'] for r in results)))
