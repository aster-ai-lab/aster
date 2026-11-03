from pathlib import Path
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

out = Path('.tmp/mise-project-uninstall-regression.json')
record = {'status': 'running', 'startedAtUnixSeconds': time.time(), 'checks': []}


def save():
    out.write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n')


def fingerprint(paths):
    values = {}
    for name in paths:
        with Path(name).open('rb') as stream:
            digest = hashlib.file_digest(stream, 'sha256').hexdigest()
        values[name] = {'bytes': Path(name).stat().st_size, 'sha256': digest}
    return values


def identities():
    result = {}
    for name in ['aster', 'aster-acp']:
        process = subprocess.run([str(Path('target/debug') / name), '--build-id'], capture_output=True, text=True)
        result[name] = {'exitCode': process.returncode, 'stdout': process.stdout, 'stderr': process.stderr}
    return result


def run(name, argv, **kwargs):
    log = Path('.tmp') / ('mise-project-uninstall-' + name + '.log')
    row = {'name': name, 'argv': [str(value) for value in argv], 'log': str(log),
           'status': 'running', 'startedAtUnixSeconds': time.time()}
    record['checks'].append(row)
    save()
    with log.open('w') as stream:
        result = subprocess.run(argv, stdout=stream, stderr=subprocess.STDOUT, **kwargs)
    row.update(status='passed' if result.returncode == 0 else 'failed',
               exitCode=result.returncode, finishedAtUnixSeconds=time.time())
    summary = re.search(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;',
                        log.read_text(encoding='utf-8'))
    if summary:
        row.update(passed=int(summary[2]), failed=int(summary[3]), ignored=int(summary[4]))
    expected = {'focused-cli': (3, 0), 'full-cli': (35, 1)}.get(name)
    if result.returncode == 0 and expected and (row.get('passed'), row.get('ignored')) != expected:
        row['status'] = 'case_count_mismatch'
        save()
        return 1
    save()
    return result.returncode


checks = json.loads(Path('.tmp/mise-project-uninstall-final-checks.json').read_text())
assert checks['status'] in ['passed', 'completed_with_known_pager_failure']
save()
sources = list(json.loads(Path('.tmp/mise-project-uninstall-work-state.json').read_text())['sources'])
artifacts = ['target/debug/aster', 'target/debug/aster-acp', 'target/debug/deps/env_manager-1f7217e7bdec0bca']
record['before'] = {'identities': identities(), 'artifacts': fingerprint(artifacts), 'sources': fingerprint(sources)}
save()
ids = record['before']['identities']
if any(value['exitCode'] for value in ids.values()) or ids['aster']['stdout'] != ids['aster-acp']['stdout']:
    record['status'] = 'pair_identity_failed'
    save()
    sys.exit(1)

test = artifacts[2]
failed = run('focused-cli', [test, 'project_uninstall', '--test-threads=1', '--nocapture'])
if not failed:
    failed = run('full-cli', [test, '--test-threads=1', '--nocapture'])

if not failed:
    aster = str(Path(artifacts[0]).resolve())
    engine = shutil.which('a3s-test')
    if not engine:
        raise RuntimeError('A3S Test is required for real interactive verification')
    suite = str(Path('tests/e2e/env-project-uninstall.acl').resolve())
    with tempfile.TemporaryDirectory(prefix='aster project confirm 中文 ') as raw:
        root = Path(raw)
        project = root / 'project'
        home = root / 'home'
        project.mkdir()
        home.mkdir()
        env = {'PATH': os.defpath, 'HOME': str(home), 'ASTER_HOME': str(home), 'NO_COLOR': '1'}
        record['interactive'] = {'selectedVersion': '0.13.6', 'project': str(project), 'home': str(home),
                                 'setupTimeoutSeconds': 120, 'scenarioTimeoutMs': 30000}
        save()
        failed = run('tui-setup', [aster, '--no-auto-update', 'tool', 'use', 'uv@0.13.6', '--json'],
                     cwd=project, env=env, timeout=120)
        if not failed:
            declaration = project / '.aster/env.acl'
            before = declaration.read_bytes()
            listing = subprocess.run([aster, '--no-auto-update', 'tool', 'ls', '--json'],
                                     cwd=project, env=env, capture_output=True, text=True, timeout=20)
            record['interactive']['beforeList'] = {'exitCode': listing.returncode, 'stdout': listing.stdout,
                                                     'stderr': listing.stderr}
            save()
            assert listing.returncode == 0
            assert [(item['name'], item['version']) for item in json.loads(listing.stdout)] == [('uv', '0.13.6')]
            args = [engine, 'run', suite, '--tui-executable', '/usr/bin/env',
                    '--tui-arg=ASTER_HOME=' + str(home), '--tui-arg=HOME=' + str(home),
                    '--tui-arg=' + aster, '--tui-arg=--no-auto-update', '--tui-arg=tool',
                    '--tui-arg=uninstall', '--tui-arg=--project',
                    '--tui-working-directory', str(project), '--max-parallel-scenarios', '1', '--json']
            failed = run('tui', args, env=env)
            listing = subprocess.run([aster, '--no-auto-update', 'tool', 'ls', '--json'],
                                     cwd=project, env=env, capture_output=True, text=True, timeout=20)
            record['interactive']['afterList'] = {'exitCode': listing.returncode, 'stdout': listing.stdout,
                                                    'stderr': listing.stderr}
            record['interactive']['declarationUnchanged'] = declaration.read_bytes() == before
            save()
            if not failed:
                tui = json.loads(Path('.tmp/mise-project-uninstall-tui.log').read_text())
                assert tui['status'] == 'passed' and len(tui['scenarios']) == 2
                assert all(item['status'] == 'passed' and item['cleanup_error'] is None for item in tui['scenarios'])
                assert listing.returncode == 0 and json.loads(listing.stdout) == []
                assert record['interactive']['declarationUnchanged']
                failed = run('damaged-real-setup', [aster, '--no-auto-update', 'tool', 'use', 'uv@0.13.6', '--json'],
                             cwd=project, env=env, timeout=120)
                if not failed:
                    located = subprocess.run([aster, '--no-auto-update', 'tool', 'which', 'uv'],
                                             cwd=project, env=env, capture_output=True, text=True, timeout=20)
                    assert located.returncode == 0, located.stderr
                    executable = Path(located.stdout.strip()).resolve()
                    installation = home / 'tools/installs/uv/0.13.6'
                    assert executable.is_relative_to(installation.resolve())
                    digest = hashlib.sha256(executable.read_bytes()).hexdigest()
                    prior = json.loads(Path('.tmp/mise-project-uninstall-real-damaged-before.json').read_text())
                    assert digest == prior['selectedExecutableSha256']
                    before = declaration.read_bytes()
                    executable.unlink()
                    failed = run('damaged-real-remove', [aster, '--no-auto-update', 'tool', 'uninstall',
                                                        '--project', '--yes', '--json'],
                                 cwd=project, env=env, timeout=20)
                    record['realDamaged'] = {'version': '0.13.6', 'executableSha256': digest,
                                             'selectedDirectoryRemains': installation.exists(),
                                             'declarationUnchanged': declaration.read_bytes() == before,
                                             'exitCode': failed}
                    save()
                    if not failed:
                        assert not record['realDamaged']['selectedDirectoryRemains']
                        assert record['realDamaged']['declarationUnchanged']
                        listing = subprocess.run([aster, '--no-auto-update', 'tool', 'ls', '--json'],
                                                 cwd=project, env=env, capture_output=True, text=True, timeout=20)
                        record['realDamaged']['afterList'] = {'exitCode': listing.returncode,
                                                              'stdout': listing.stdout, 'stderr': listing.stderr}
                        save()
                        assert listing.returncode == 0 and json.loads(listing.stdout) == []

record['after'] = {'identities': identities(), 'artifacts': fingerprint(artifacts), 'sources': fingerprint(sources)}
record['unchanged'] = record['before'] == record['after']
record.update(status='passed' if not failed and record['unchanged'] else 'failed',
              finishedAtUnixSeconds=time.time())
save()
print(record['status'])
sys.exit(0 if record['status'] == 'passed' else 1)
