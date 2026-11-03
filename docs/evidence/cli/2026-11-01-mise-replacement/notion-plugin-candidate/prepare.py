from pathlib import Path
import datetime
import hashlib
import json
import shutil
import subprocess
import tarfile
import zipfile

root = Path.cwd()
temporary = root / '.tmp/plugin-registry'
candidate = temporary / 'ntn-aix-candidate-v3'
assert not candidate.exists(), 'candidate output must be new'
metadata = json.loads((temporary / 'ntn-latest-metadata.json').read_text())
inventory = json.loads((temporary / 'ntn-package-inventory.json').read_text())
archive = temporary / 'ntn-upstream-package.tgz'
assert inventory['sha512IntegrityVerified']
assert hashlib.sha256(archive.read_bytes()).hexdigest() == inventory['archiveSha256']
assert metadata['name'] == 'ntn' and metadata['version'] == inventory['version']
version = metadata['version']
builder = temporary / 'current-plugin-packages-v2/bin/aster'
authoring = candidate / 'authoring'
package = authoring / 'plugins/productivity/aster/ntn'
package.mkdir(parents=True)
policy = (temporary / 'published-official-source/domains.acl').read_text()
policy = policy.rstrip()[:-1] + '''  domain "productivity" {
    owners = ["aster-ai-lab"]
    publishers = ["aster"]
  }
}
'''
(authoring / 'domains.acl').write_text(policy)
with tarfile.open(archive, 'r:gz') as upstream:
    license_text = upstream.extractfile('package/LICENSE.md').read().decode('utf-8')
    members = {member.name for member in upstream.getmembers() if member.isfile()}
own_license = (root / 'apps/use-registry/LICENSE').read_text()
(package / 'LICENSE').write_text(own_license)
# Exact producer mappings from the package's install.cjs; platforms do not change the package ID.
variants = [
    ('darwin-arm64', 'aarch64-macos-', 'ntn-darwin-arm64', 'ntn'),
    ('darwin-x64', 'x86_64-macos-', 'ntn-darwin-x64', 'ntn'),
    ('linux-arm64', 'aarch64-linux-gnu', 'ntn-linux-arm64', 'ntn'),
    ('linux-x64', 'x86_64-linux-gnu', 'ntn-linux-x64', 'ntn'),
    ('windows-x64', 'x86_64-windows-msvc', 'ntn-win32-x64', 'ntn.exe'),
]
(package / 'README.md').write_text(f'''# Notion CLI

`aster/ntn` {version} references the immutable upstream npm tarball. Each
`variants/<target>` directory is a native AIX build source with the same package
ID. Platform selection follows the producer's explicit `install.cjs` table.
The acquisition declaration installs the native command from `dist/`, retaining
the complete tarball and upstream license. It does not execute install scripts.

Publication requires the desktop toolchain lifecycle adapter and remote registry
acceptance. These candidate artifacts are not a production publication.
''')
results = []
for target, platform, distribution, executable in variants:
    entry = f'dist/{distribution}/{executable}'
    assert f'package/{entry}' in members
    source = package / 'variants' / target
    source.mkdir(parents=True)
    (source / 'aster-use-extension.acl').write_text(f'''extension "aster/ntn" {{
  schema_version = 3
  version = "{version}"
  route = "ntn"
  requires_use = ">=0.3.0, <0.4.0"
  actions = ["read"]
  repository {{ url = "https://github.com/aster-ai-lab/plugins" }}
  plugin "toolchain" {{ entry = "toolchain.acl" optional = false }}
}}
''')
    (source / 'toolchain.acl').write_text(f'''toolchain {{
  schema_version = 1
  name = "ntn"
  spec = "npm:ntn"
  version = "{version}"
  executable = "ntn"
  platform = "{platform}"
  url = "{metadata['dist']['tarball']}"
  length = {inventory['archiveBytes']}
  sha256 = "{inventory['archiveSha256']}"
  format = "tar.gz"
  prefix = "package"
  bin_dirs = ["dist/{distribution}"]
  executable_mode = "0755"
  command "ntn" {{ path = "{entry}" }}
}}
''')
    (source / 'README.md').write_text((package / 'README.md').read_text())
    (source / 'LICENSE').write_text(own_license)
    (source / 'UPSTREAM-LICENSE.md').write_text(license_text)
    repository = candidate / 'repository' / target
    staged_source = repository / 'plugins/productivity/aster/ntn'
    shutil.copytree(source, staged_source)
    (repository / 'domains.acl').write_text(policy)
    output = candidate / 'artifacts' / f'ntn-{version}-{target}.aix'
    output.parent.mkdir(exist_ok=True)
    commands = [
        [str(builder), '--no-auto-update', 'use', 'registry', 'package', 'build', str(staged_source), '--output', str(output), '--json'],
        [str(builder), '--no-auto-update', 'use', 'registry', 'package', 'check', str(output), '--json'],
        [str(builder), '--no-auto-update', 'use', 'registry', 'package', 'prepare', str(output), '--repository', str(repository), '--domain', 'productivity', '--output', str(output.with_suffix('.oci.json')), '--json'],
    ]
    checks = []
    for index, argv in enumerate(commands):
        result = subprocess.run(argv, capture_output=True, text=True, timeout=60)
        (candidate / f'{target}-{index}.json').write_text(json.dumps({'argv': argv, 'exitCode': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr}, indent=2) + '\n')
        checks.append(result.returncode)
        if result.returncode:
            break
    row = {'target': target, 'platform': platform, 'entry': entry, 'checks': checks, 'passed': checks == [0, 0, 0]}
    if output.exists():
        row['archiveSha256'] = hashlib.sha256(output.read_bytes()).hexdigest()
        with zipfile.ZipFile(output) as aix:
            for name in ['aster-use-extension.acl', 'toolchain.acl', 'README.md', 'LICENSE', 'UPSTREAM-LICENSE.md']:
                assert aix.read(name) == (source / name).read_bytes()
            row['sourceFilesMatchFinalArchive'] = True
            if target == 'darwin-arm64':
                (candidate / 'native-toolchain.acl').write_bytes(aix.read('toolchain.acl'))
    results.append(row)
    report = {'capturedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'packageId': 'aster/ntn', 'version': version, 'artifactFormat': 'aix', 'upstreamArchiveSha256': inventory['archiveSha256'], 'results': results, 'publicationAccepted': False, 'desktopLifecycleAccepted': False}
    (candidate / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(row), flush=True)
