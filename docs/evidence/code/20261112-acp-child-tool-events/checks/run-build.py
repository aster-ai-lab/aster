import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

workspace = Path('/Users/roylin/code/kense')
metadata = workspace / 'krun/.build/aster-child-events-build.json'
build = json.loads(metadata.read_text())
stage = Path(build['stage'])
assert stage.parent == workspace / 'krun/.build'
assert Path(build['kboxHome']) == stage / 'kbox-home'
cache = workspace / 'kbox/.tmp/acp-harnesses'
name = 'aster-child-events-compile-' + stage.name[-12:]
kbox = str(Path(build['prefix']) / 'bin/kbox')
environment = {key: os.environ[key] for key in ['PATH', 'HOME', 'TMPDIR'] if key in os.environ}
environment['K_HOME'] = build['kboxHome']
assert json.loads(subprocess.check_output([kbox, 'ps', '-a', '--format', 'json'], env=environment)) == []
command = [kbox, 'run', '--rm', '-i', '--name', name, '--cpus', '4', '--memory', '24g',
           '--entrypoint', 'bash', '--workdir', '/src']
for host, guest in [(Path(build['source']), '/src'), (cache / 'aster-cargo/bin', '/cargo/bin'),
                    (cache / 'aster-cargo/registry', '/cargo/registry'), (cache / 'aster-cargo/git', '/cargo/git'),
                    (cache / 'aster-rustup', '/rustup'), (cache / 'aster-target', '/target'),
                    (stage / 'artifacts', '/out')]:
    assert host.is_dir(), host
    command += ['-v', f'{host}:{guest}']
command += [build['builderImage'], '--', '-s']
(stage / 'compile-command.json').write_text(json.dumps(dict(command=command, script=str(stage / 'build.sh'),
    sourceSnapshotDigest=build['sourceSnapshotDigest'], K_HOME=build['kboxHome']), indent=2) + '\n')
(stage / 'compile-exit.json').unlink(missing_ok=True)
started = time.monotonic()
print('building', name, flush=True)
try:
    with (stage / 'compile.stdout').open('wb') as output, (stage / 'compile.stderr').open('wb') as errors:
        result = subprocess.run(command, input=(stage / 'build.sh').read_bytes(), env=environment,
                                stdout=output, stderr=errors, timeout=7200)
    (stage / 'compile-exit.json').write_text(json.dumps(dict(exitCode=result.returncode,
        seconds=round(time.monotonic() - started, 2))) + '\n')
    print((stage / 'compile-exit.json').read_text(), flush=True)
    if result.returncode:
        print((stage / 'compile.stderr').read_text(errors='replace')[-2000:], flush=True)
        raise SystemExit(result.returncode)
    archive = stage / 'artifacts/aster-cli-linux-arm64.tar.gz'
    build.update(archive=str(archive), archiveSha256=hashlib.sha256(archive.read_bytes()).hexdigest())
    metadata.write_text(json.dumps(build, indent=2) + '\n')
    print('archive ready', archive, flush=True)
finally:
    for action, label in [(['rm', '-f', name], 'rm'), (['ps', '-a', '--format', 'json'], 'ps')]:
        cleanup = subprocess.run([kbox, *action], env=environment, capture_output=True, text=True, timeout=60)
        (stage / ('compile-cleanup-' + label + '.json')).write_text(json.dumps(dict(exitCode=cleanup.returncode,
            stdout=cleanup.stdout, stderr=cleanup.stderr), indent=2) + '\n')
    assert cleanup.returncode == 0 and json.loads(cleanup.stdout) == []
