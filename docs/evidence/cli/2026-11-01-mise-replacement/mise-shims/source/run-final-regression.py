from pathlib import Path
import hashlib
import json
import os
import re
import subprocess
import sys
import time

result_path = Path('.tmp/mise-shims-final-regression.json')
record = {'status': 'running', 'startedAtUnixSeconds': time.time(), 'checks': []}


def save():
    result_path.write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n')


def identities():
    values = {}
    for name in ['aster', 'aster-acp']:
        process = subprocess.run([str(Path('target/debug') / name), '--build-id'],
                                 capture_output=True, text=True, encoding='utf-8')
        values[name] = {'exitCode': process.returncode, 'stdout': process.stdout,
                        'stderr': process.stderr}
    return values


def files(paths):
    result = {}
    for path in paths:
        with Path(path).open('rb') as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        result[path] = {'bytes': Path(path).stat().st_size, 'sha256': digest}
    return result


save()
assert json.loads(Path('.tmp/mise-shims-clean-source-checks.json').read_text())['status'] == 'passed'
sources = list(json.loads(Path('.tmp/mise-shims-work-state.json').read_text())['sourceSha256'])
artifacts = ['target/debug/aster', 'target/debug/aster-acp',
             'target/debug/deps/env_manager-1f7217e7bdec0bca']
record['before'] = {'identities': identities(), 'artifacts': files(artifacts), 'sources': files(sources)}
save()
ids = record['before']['identities']
if any(value['exitCode'] for value in ids.values()) or ids['aster']['stdout'] != ids['aster-acp']['stdout']:
    record['status'] = 'initial_pair_mismatch'
    save()
    sys.exit(1)

test = artifacts[2]
checks = [
    ('full-cli-env', [test, '--test-threads=1', '--nocapture'], {}),
    ('real-uv-auto-install', [test, 'project_shim_installs_its_missing_declared_version_from_the_real_backend',
                             '--ignored', '--exact', '--test-threads=1', '--nocapture'],
     {'ASTER_TEST_SHIM_AUTO_VERSION': '0.13.6',
      'ASTER_TEST_SHIM_AUTO_RESULT': str(Path('.tmp/mise-shims-uv-auto-install-final-raw.json').resolve())}),
]
for name, args, selected_env in checks:
    log = Path('.tmp') / f'mise-shims-{name}-final.log'
    row = {'name': name, 'argv': args, 'log': str(log), 'status': 'running',
           'startedAtUnixSeconds': time.time()}
    record['checks'].append(row)
    save()
    with log.open('w') as stream:
        code = subprocess.run(args, env={**os.environ, **selected_env}, stdout=stream,
                              stderr=subprocess.STDOUT).returncode
    row.update(status='passed' if code == 0 else 'failed', exitCode=code,
               finishedAtUnixSeconds=time.time())
    summary = re.search(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;',
                        log.read_text(encoding='utf-8'))
    if summary:
        row.update(passed=int(summary[2]), failed=int(summary[3]), ignored=int(summary[4]))
    save()
    if code:
        break

record['after'] = {'identities': identities(), 'artifacts': files(artifacts), 'sources': files(sources)}
record['unchanged'] = record['before'] == record['after']
record['status'] = ('passed' if record['unchanged'] and len(record['checks']) == len(checks)
                    and all(row.get('exitCode') == 0 for row in record['checks']) else 'failed')
record['finishedAtUnixSeconds'] = time.time()
save()
print(record['status'])
sys.exit(0 if record['status'] == 'passed' else 1)
