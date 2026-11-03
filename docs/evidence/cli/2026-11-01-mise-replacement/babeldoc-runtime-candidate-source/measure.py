from pathlib import Path
import datetime
import hashlib
import json
import os
import shutil
import subprocess
import tarfile

root = Path.cwd()
inputs = root / '.tmp/plugin-registry/babeldoc-measurement'
out = root / '.tmp/plugin-registry/babeldoc-compatible-v2'
bundle = out / 'owned-runtime'
assert not bundle.exists()
selected = json.loads((inputs / 'python-selected.json').read_text())
interpreter = inputs / 'owned-python' / selected['key']
shutil.copytree(interpreter, bundle / 'python', symlinks=True)
shutil.copytree(out / 'site-packages', bundle / 'site-packages', symlinks=True)
(bundle / 'bin').mkdir()
launcher = bundle / 'bin/babeldoc-stream'
launcher.write_text('''#!/bin/sh
runtime_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd) || exit 1
exec "$runtime_root/python/bin/python3.12" -I -B "$runtime_root/launcher.py" "$@"
''', encoding='utf-8')
launcher.chmod(0o755)
(bundle / 'launcher.py').write_text('''from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent / "site-packages"))
sys.argv[0] = "babeldoc-stream"
from babeldoc.main import cli

cli()
''', encoding='utf-8')
shutil.copyfile(out / 'requirements.lock', bundle / 'requirements.lock')
entries = list(bundle.rglob('*'))
files = [path for path in entries if path.is_file() and not path.is_symlink()]
links = [path for path in entries if path.is_symlink()]
for link in links:
    target = os.readlink(link)
    assert not Path(target).is_absolute(), (link, target)
    assert link.resolve().is_relative_to(bundle.resolve()), (link, target)
argv = [str(bundle / 'python/bin/python3.12'), '-I', '-B', '-c', 'import json,sys; print(json.dumps({"prefix":sys.prefix,"version":sys.version}))']
process = subprocess.run(argv, capture_output=True, text=True, timeout=60)
assert process.returncode == 0, process.stderr
native = json.loads(process.stdout)
assert Path(native['prefix']).resolve() == (bundle / 'python').resolve(), native
version_argv = [str(launcher), '--version']
version_process = subprocess.run(version_argv, capture_output=True, text=True)
assert version_process.returncode == 0, version_process.stderr
assert version_process.stdout.strip() == 'babeldoc-stream 0.6.4.post4', version_process.stdout
(out / 'owned-runtime-version.log').write_text(version_process.stdout + version_process.stderr)
archive = out / 'owned-runtime-darwin-arm64.tar.gz'
with tarfile.open(archive, 'w:gz') as output:
    output.add(bundle, arcname='babeldoc-stream')
report = {
    'capturedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'platform': 'darwin-arm64', 'python': selected,
    'files': len(files), 'symlinks': len(links), 'treeEntries': len(entries),
    'expandedFileBytes': sum(path.stat().st_size for path in files),
    'archiveBytes': archive.stat().st_size,
    'archiveSha256': hashlib.sha256(archive.read_bytes()).hexdigest(),
    'requirementsSha256': hashlib.sha256((out / 'requirements.lock').read_bytes()).hexdigest(),
    'includesOwnedPythonRuntime': True, 'relocatedInterpreter': native,
    'interpreterExecuted': True, 'babeldocCommandExecuted': True,
    'versionCommand': version_argv, 'versionOutput': version_process.stdout.strip(),
    'translationAccepted': False,
    'aixBuilt': False, 'published': False,
}
(out / 'owned-runtime-measurement.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report), flush=True)
