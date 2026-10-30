"""Reprobe exact selected cases with an existing immutable native harness/pair."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('platform', choices=('windows', 'linux'))
parser.add_argument('source_phase')
parser.add_argument('destination_phase')
parser.add_argument('--config', type=Path, required=True)
parser.add_argument('--expected-tests', type=int, required=True)
parser.add_argument('tests', nargs='+')
args = parser.parse_args()
root = Path.cwd().resolve()
campaign = Path(__file__).resolve().parent.parent
source = campaign / args.platform / args.source_phase
destination = campaign / args.platform / args.destination_phase
assert not destination.exists(), 'retain earlier evaluation artifacts'
destination.mkdir(parents=True)
pair = json.loads(Path((source / 'manifest-path.txt').read_text().strip()).read_text())
harness_path = source / 'test-binary.json'
harness = json.loads(harness_path.read_text())
assert harness['source_pair'] == pair['source_pair']
assert harness['entrypoint_count'] == len(set(harness['entrypoints'])) == args.expected_tests
assert set(args.tests).issubset(harness['entrypoints'])


def sha(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


assert sha(Path(harness['binary'])) == harness['sha256']
for item in pair['files']:
    binary = Path(pair['folder']) / item['name']
    assert binary.stat().st_size == item['bytes'] and sha(binary) == item['sha256']
    assert subprocess.check_output([str(binary), '--build-id'], text=True).strip() == pair['source_pair']
suffix = '.exe' if args.platform == 'windows' else ''
config = args.config.resolve()
config_sha = sha(config)
env = os.environ.copy()
env.update({
    'A3S_CONFIG_FILE': str(config),
    'A3S_CODE_TUI_BIN': str(Path(pair['folder']) / ('aster' + suffix)),
    'A3S_CODE_ACP_BIN': str(Path(pair['folder']) / ('a3s-code-acp' + suffix)),
    'A3S_CODE_ACP_TRACE_PROXY': str(root / '.tmp' / ('acp-trace-proxy.exe' if suffix else 'acp-trace-proxy-linux')),
    'A3S_TUI_LIVE_EVIDENCE_DIR': str(destination / 'reports'),
})
invocation = {'source_pair': pair['source_pair'], 'harness_manifest': harness_path.relative_to(campaign).as_posix(),
              'harness_manifest_sha256': sha(harness_path), 'binary_sha256': harness['sha256'],
              'model': 'kimi/k3.1-pro', 'script_sha256': sha(Path(__file__)),
              'tests': args.tests, 'expected_tests': args.expected_tests}
(destination / 'invocation.json').write_text(json.dumps(invocation, indent=2) + '\n', encoding='utf-8', newline='\n')
results = []
for test in args.tests:
    log = destination / (test.rpartition('::')[-1] + '.log')
    with log.open('wb') as stream:
        code = subprocess.run([harness['binary'], test, '--ignored', '--exact', '--nocapture', '--test-threads=1'],
                              env=env, stdout=stream, stderr=subprocess.STDOUT).returncode
    assert sha(config) == config_sha
    results.append({'test': test, 'exit_code': code, 'log': log.relative_to(campaign).as_posix()})
    (destination / 'feature-results.json').write_text(
        json.dumps({'source_pair': pair['source_pair'], 'config_unchanged': True, 'tests': results}, indent=2) + '\n',
        encoding='utf-8', newline='\n')
    print(json.dumps(results[-1]), flush=True)
raise SystemExit(int(any(result['exit_code'] for result in results)))
