from pathlib import Path
import hashlib
import json
import shutil
import subprocess
import zipfile

root = Path.cwd()
temporary = root / '.tmp/plugin-registry'
original = temporary / 'cua-aix-candidate'
output = temporary / 'cua-aix-candidate-v3'
assert not output.exists(), 'candidate output must be new'
shutil.copytree(original / 'authoring', output / 'authoring')
source = output / 'authoring/plugins/operations/aster/cua/variants/darwin-arm64'
manifest = source / 'aster-use-extension.acl'
text = manifest.read_text(encoding='utf-8')
assert text.count('requires_use = ">=0.3.0, <0.4.0"') == 1
manifest.write_text(text.replace('requires_use = ">=0.3.0, <0.4.0"', 'requires_use = ">=0.3.4, <0.4.0"'), encoding='utf-8')
repository = output / 'repository/darwin-arm64'
staged = repository / 'plugins/operations/aster/cua'
shutil.copytree(source, staged)
shutil.copyfile(original / 'repository/darwin-arm64/domains.acl', repository / 'domains.acl')
aix = output / 'artifacts/cua-0.9.0-darwin-arm64.aix'
aix.parent.mkdir()
builder = str(root / 'target/debug/aster-use')
commands = [
    [builder, 'registry', 'package', 'build', str(staged), '--output', str(aix), '--json'],
    [builder, 'registry', 'package', 'check', str(aix), '--json'],
    [builder, 'registry', 'package', 'prepare', str(aix), '--repository', str(repository), '--domain', 'operations', '--output', str(aix.with_suffix('.oci.json')), '--json'],
]
for index, argv in enumerate(commands):
    result = subprocess.run(argv, capture_output=True, text=True, timeout=60)
    (output / f'check-{index}.json').write_text(json.dumps({'argv': argv, 'exitCode': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr}, indent=2) + '\n')
    assert result.returncode == 0, (argv, result.stdout, result.stderr)
with zipfile.ZipFile(aix) as archive:
    expected = {p.relative_to(source).as_posix(): p.read_bytes() for p in source.rglob('*') if p.is_file()}
    actual = {name: archive.read(name) for name in archive.namelist() if not name.endswith('/')}
    assert actual == expected
report = json.loads((original / 'validation.json').read_text())
report['aixArchiveSha256'] = 'sha256:' + hashlib.sha256(aix.read_bytes()).hexdigest()
report['requiresUse'] = '>=0.3.4, <0.4.0'
(output / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
inputs = json.loads((original / 'native-inputs.json').read_text())
inputs[0]['aix'] = str(aix.resolve())
(output / 'native-inputs.json').write_text(json.dumps(inputs, indent=2) + '\n')
print(json.dumps(report), flush=True)
