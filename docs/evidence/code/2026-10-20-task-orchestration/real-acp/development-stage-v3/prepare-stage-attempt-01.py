#!/usr/bin/env python3
"""Prepare only the explicitly authorized development app; never launch it."""
import datetime
import hashlib
import json
import os
import pathlib
import plistlib
import shutil
import stat
import subprocess
import time

ROOT = pathlib.Path('/Users/roylin/code/aster')
EVIDENCE = pathlib.Path(__file__).resolve().parent
BASE = EVIDENCE.parent.parent
PREP = BASE / 'real-acp/development-stage-preparation.json'
BUILD = BASE / 'final-build-v3/builds.json'
prep = json.loads(PREP.read_text())
build = json.loads(BUILD.read_text())
app = pathlib.Path(prep['app'])
assert app == ROOT / 'target/development-stage/task-orchestration-20261020/AsterDream-TaskOrchestration-Dev.app'
assert app.resolve() == app and not app.is_symlink()
macos = app / 'Contents/MacOS'
assert list(macos.iterdir()) == [], 'refuse to overwrite an existing stage executable'


def sha(path):
    h = hashlib.sha256()
    with pathlib.Path(path).open('rb') as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def identity(path):
    path = pathlib.Path(path)
    s = path.lstat()
    assert stat.S_ISREG(s.st_mode), 'expected a regular file'
    return {'path': str(path), 'sha256': sha(path), 'bytes': s.st_size,
            'mode': oct(stat.S_IMODE(s.st_mode))}


report = {'schema': 'aster.development-stage-signing.v3',
          'started_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'app': str(app), 'state': 'preparing', 'launched': False,
          'preparation_sha256': sha(PREP), 'build_report_sha256': sha(BUILD),
          'script_sha256': sha(__file__), 'commands': [], 'binaries': {}}


def save():
    (EVIDENCE / 'stage.json').write_text(json.dumps(report, indent=2) + '\n')


def command(name, argv):
    started = time.monotonic()
    result = subprocess.run(argv, capture_output=True, text=True)
    stdout, stderr = EVIDENCE / (name + '.stdout.log'), EVIDENCE / (name + '.stderr.log')
    stdout.write_text(result.stdout)
    stderr.write_text(result.stderr)
    report['commands'].append({'name': name, 'command': argv,
                              'exit_code': result.returncode,
                              'elapsed_ms': round((time.monotonic() - started) * 1000, 3),
                              'stdout': str(stdout), 'stderr': str(stderr)})
    save()
    if result.returncode:
        report['state'] = 'failed'
        save()
        raise RuntimeError('stage command failed: ' + name)
    return result.stdout + result.stderr


def signature(name, path):
    output = command(name, ['/usr/bin/codesign', '-d', '--verbose=4', str(path)])
    fields = dict(line.split('=', 1) for line in output.splitlines() if '=' in line)
    assert fields.get('Signature') == 'adhoc' and fields.get('CDHash'), 'missing adhoc signature identity'
    return {key: fields[key] for key in ['Identifier', 'Signature', 'CDHash']}


old = pathlib.Path('/Users/roylin/Library/Application Support/dev.a3s.studio/aster/AsterDream.app')
old_paths = [old / 'Contents/MacOS/AsterDream', old / 'Contents/Info.plist']
report['old_app_before'] = [identity(path) for path in old_paths]
acl = pathlib.Path(prep['host_capabilities']['path'])
report['host_capabilities_before'] = identity(acl)
assert report['host_capabilities_before']['sha256'] == prep['host_capabilities']['sha256']
assert stat.S_IMODE(acl.stat().st_mode) == 0o600
plist_path = app / 'Contents/Info.plist'
plist_before = plistlib.loads(plist_path.read_bytes())
assert plist_before['LSEnvironment'] == prep['environment']
identifier = plist_before['CFBundleIdentifier']
assert identifier == 'dev.a3s.studio.aster.task-orchestration-dev'
report['plist_before'] = identity(plist_path)
report['plist_values_before'] = plist_before
report['template_source'] = identity(ROOT / 'apps/work/src-tauri/src/macos_app.rs')
report['icon_before'] = identity(app / 'Contents/Resources/AsterDream.icns')
assert report['icon_before']['sha256'] == prep['icon_sha256']
save()

for target, filename in [('aster-dream', 'AsterDream'), ('a3s-code-acp', 'a3s-code-acp'), ('a3s-office', 'a3s-office')]:
    source = pathlib.Path(build['final_executables'][target]['path'])
    before = identity(source)
    assert before['sha256'] == build['final_executables'][target]['sha256'], 'final build binary changed'
    destination = macos / filename
    shutil.copyfile(source, destination)
    destination.chmod(0o755)
    copied = identity(destination)
    assert copied['sha256'] == before['sha256']
    report['binaries'][target] = {'source_before': before, 'copied_unsigned': copied,
                                'copy_operation': 'shutil.copyfile(source,destination); chmod(0755)'}
    save()

plist_after = dict(plist_before)
plist_after.update({'CFBundleInfoDictionaryVersion': '6.0',
                    'NSAppTransportSecurity': {'NSAllowsArbitraryLoads': True,
                                               'NSAllowsArbitraryLoadsInWebContent': True},
                    'NSHighResolutionCapable': True, 'NSPrincipalClass': 'NSApplication'})
assert plist_after['LSEnvironment'] == plist_before['LSEnvironment']
plist_path.write_bytes(plistlib.dumps(plist_after, fmt=plistlib.FMT_XML, sort_keys=False))
report['plist_completed'] = identity(plist_path)
report['plist_values_completed'] = plist_after
command('plist-lint', ['/usr/bin/plutil', '-lint', str(plist_path)])
for target, suffix in [('a3s-code-acp', 'acp'), ('a3s-office', 'office')]:
    destination = pathlib.Path(report['binaries'][target]['copied_unsigned']['path'])
    command('sign-' + suffix, ['/usr/bin/codesign', '--force', '--sign', '-',
                             '--identifier', identifier + '.' + suffix, str(destination)])
    report['binaries'][target]['signed_inner'] = identity(destination)
    report['binaries'][target]['inner_signature'] = signature('signature-' + suffix, destination)
    save()
command('sign-app', ['/usr/bin/codesign', '--force', '--sign', '-', '--identifier', identifier, str(app)])
command('verify-app-strict-deep', ['/usr/bin/codesign', '--verify', '--deep', '--strict', '--verbose=2', str(app)])
report['app_signature'] = signature('signature-app', app)
for target, item in report['binaries'].items():
    destination = pathlib.Path(item['copied_unsigned']['path'])
    item['signed_final'] = identity(destination)
    item['final_signature'] = signature('signature-final-' + target, destination)
    item['source_after'] = identity(item['source_before']['path'])
    assert item['source_after'] == item['source_before'], 'source executable changed'
report['old_app_after'] = [identity(path) for path in old_paths]
assert report['old_app_after'] == report['old_app_before'], 'old app was modified'
report['host_capabilities_after'] = identity(acl)
assert report['host_capabilities_after'] == report['host_capabilities_before']
report['icon_after'] = identity(app / 'Contents/Resources/AsterDream.icns')
assert report['icon_after'] == report['icon_before']
assert not any(path.is_symlink() for path in app.rglob('*')), 'stage must contain no symlinks'
report['state'] = 'signed_strict_verified_not_launched'
report['external_assets'] = {'path': prep['environment']['ASTER_SANDBOX_DIR'],
                             'modified': False, 'signed': False,
                             'scope': 'Existing frozen directory referenced only by per-app LSEnvironment'}
report['ended_at_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
save()
print(json.dumps({'state': report['state'], 'app': str(app),
                  'binaries': {name: item['signed_final'] for name, item in report['binaries'].items()}}))
