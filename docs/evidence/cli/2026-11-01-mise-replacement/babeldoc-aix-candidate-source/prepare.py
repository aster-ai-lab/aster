from pathlib import Path
import datetime
import email.parser
import gzip
import hashlib
import io
import json
import shutil
import subprocess
import tarfile
import time
import zipfile

root = Path.cwd()
temporary = root / '.tmp/plugin-registry'
inputs = temporary / 'babeldoc-compatible-v2'
output = temporary / 'babeldoc-aix-candidate'
assert not output.exists()
output.mkdir()
verified = json.loads((inputs / 'bundle-verification.json').read_text())
assert verified['archiveSourceTreeMatches'] and verified['allOtherResolvedVersionsUnchanged']
bundle = inputs / 'owned-runtime'
bootstrap = (temporary / 'babeldoc-bootstrap.py').read_bytes()
runtime_archive = output / 'babeldoc-stream-darwin-arm64.tar.gz'
file_count = byte_count = link_count = 0
with runtime_archive.open('wb') as raw:
    with gzip.GzipFile(filename='', fileobj=raw, mode='wb', mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode='w', format=tarfile.PAX_FORMAT) as archive:
            for path in [bundle, *sorted(bundle.rglob('*'))]:
                relative = path.relative_to(bundle).as_posix()
                name = 'babeldoc-stream' if relative == '.' else 'babeldoc-stream/' + relative
                info = archive.gettarinfo(str(path), arcname=name)
                info.uid = info.gid = 0
                info.uname = info.gname = ''
                info.mtime = 0
                if info.isfile():
                    if relative == 'launcher.py':
                        info.size = len(bootstrap)
                        archive.addfile(info, io.BytesIO(bootstrap))
                    else:
                        with path.open('rb') as stream:
                            archive.addfile(info, stream)
                    file_count += 1
                    byte_count += info.size
                else:
                    assert info.isdir() or info.issym(), path
                    link_count += int(info.issym())
                    archive.addfile(info)
assert file_count == verified['regularFiles'] and link_count == verified['symlinks']
digest = hashlib.sha256(runtime_archive.read_bytes()).hexdigest()
length = runtime_archive.stat().st_size
assert length <= 512 * 1024 * 1024 and byte_count < 4 * 1024**3
source = output / 'authoring/plugins/knowledge/aster/babeldoc-stream/variants/darwin-arm64'
(source / 'payload').mkdir(parents=True)
shutil.copyfile(runtime_archive, source / 'payload/babeldoc-stream-darwin-arm64.tar.gz')
metadata_path = inputs / 'site-packages/babeldoc_stream-0.6.4.post4.dist-info/METADATA'
metadata = email.parser.BytesParser().parsebytes(metadata_path.read_bytes())
assert metadata['Name'] == 'babeldoc-stream' and metadata['Version'] == '0.6.4.post4'
version = '0.1.0'
(source / 'aster-use-extension.acl').write_text(f'''extension "aster/babeldoc-stream" {{
 schema_version = 3
 version = "{version}"
 route = "babeldoc-stream"
 requires_use = ">=0.3.4, <0.4.0"
 actions = ["read"]
 repository {{ url = "https://github.com/aster-ai-lab/plugins" }}
 plugin "toolchain" {{ entry = "toolchain.acl" optional = false }}
}}
''')
(source / 'toolchain.acl').write_text(f'''toolchain {{
 schema_version = 1
 name = "babeldoc-stream"
 spec = "pipx:babeldoc-stream"
 version = "{metadata['Version']}"
 executable = "babeldoc-stream"
 platform = "aarch64-macos-"
 archive = "payload/babeldoc-stream-darwin-arm64.tar.gz"
 length = {length}
 sha256 = "{digest}"
 format = "tar.gz"
 prefix = "babeldoc-stream"
 bin_dirs = ["bin"]
 executable_mode = "0755"
 command "babeldoc-stream" {{ path = "bin/babeldoc-stream" }}
}}
''')
legal = metadata_path.parent / 'licenses'
shutil.copyfile(legal / 'LICENSE', source / 'LICENSE')
shutil.copyfile(legal / 'NOTICE', source / 'NOTICE')
shutil.copyfile(root / 'apps/use-registry/LICENSE', source / 'ASTER-RECIPE-LICENSE')
(source / 'README.md').write_text(f'''# BabelDOC Stream

`aster/babeldoc-stream` recipe {version} embeds BabelDOC Stream {metadata['Version']}
and its own CPython 3.12.14. The exact runtime archive is bound by SHA-256;
installation does not invoke pipx, uv or an upstream installer. The source
identity remains `pipx:babeldoc-stream`; the recipe version is independent.

The upstream license expression is `{metadata['License-Expression']}`. LICENSE
and NOTICE are copied verbatim, and the archive retains Python and every wheel's
notices. ASTER-RECIPE-LICENSE covers Aster's acquisition recipe and launcher.
The pinned wheel lock constrains bitstring to <5 because BabelDOC uses BitStream.
The launcher uses the original `babeldoc.main:cli` with an isolated owned Python.

This macOS ARM64 candidate is not an official publication. Version startup does
not establish PDF translation, native GUI acceptance or other-target support.
''')
provenance = {
    'upstreamName': metadata['Name'], 'upstreamVersion': metadata['Version'],
    'upstreamLicenseExpression': metadata['License-Expression'],
    'projectUrls': metadata.get_all('Project-URL'),
    'requirementsSha256': hashlib.sha256((inputs / 'requirements.lock').read_bytes()).hexdigest(),
    'bootstrapSha256': hashlib.sha256(bootstrap).hexdigest(),
    'python': json.loads((temporary / 'babeldoc-measurement/python-selected.json').read_text()),
}
(source / 'SOURCE.json').write_text(json.dumps(provenance, indent=2) + '\n')
policy = '''plugin_domains {
 schema_version = 1
 domain "knowledge" { owners = ["aster-ai-lab"] publishers = ["aster"] }
}
'''
repository = output / 'repository/darwin-arm64'
staged = repository / 'plugins/knowledge/aster/babeldoc-stream'
shutil.copytree(source, staged)
(repository / 'domains.acl').write_text(policy)
(output / 'authoring/domains.acl').write_text(policy)
aix = output / f'babeldoc-stream-{version}-darwin-arm64.aix'
builder = str(root / 'target/debug/aster-use')
commands = [
    [builder, 'registry', 'package', 'build', str(staged), '--output', str(aix), '--json'],
    [builder, 'registry', 'package', 'check', str(aix), '--json'],
    [builder, 'registry', 'package', 'prepare', str(aix), '--repository', str(repository), '--domain', 'knowledge', '--output', str(aix.with_suffix('.oci.json')), '--json'],
]
checks = []
for index, argv in enumerate(commands):
    started = time.monotonic()
    process = subprocess.run(argv, capture_output=True, text=True)
    check = {'argv': argv, 'timeoutSeconds': None, 'exitCode': process.returncode, 'seconds': time.monotonic() - started, 'stdout': process.stdout, 'stderr': process.stderr}
    (output / f'check-{index}.json').write_text(json.dumps(check, indent=2) + '\n')
    checks.append(check)
    report = {'capturedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'packageId': 'aster/babeldoc-stream', 'recipeVersion': version, 'toolVersion': metadata['Version'], 'target': 'darwin-arm64', 'runtimeArchiveSha256': digest, 'runtimeArchiveLength': length, 'runtimeRegularFiles': file_count, 'runtimeSymlinks': link_count, 'runtimeExpandedFileBytes': byte_count, 'checks': checks, 'published': False, 'nativeManagementAccepted': False}
    (output / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
    print(index, process.returncode, flush=True)
    assert process.returncode == 0
with zipfile.ZipFile(aix) as archive:
    assert set(archive.namelist()) == {p.relative_to(source).as_posix() for p in source.rglob('*') if p.is_file()}
    for name in archive.namelist():
        assert archive.read(name) == (source / name).read_bytes(), name
report['sourceFilesMatchFinalArchive'] = True
report['aixArchiveSha256'] = hashlib.sha256(aix.read_bytes()).hexdigest()
(output / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
(output / 'native-inputs.json').write_text(json.dumps([{'packageId': 'aster/babeldoc-stream', 'aix': str(aix.resolve()), 'archive': str(runtime_archive.resolve()), 'version': metadata['Version'], 'tool': 'babeldoc-stream', 'versionArgs': ['--version'], 'requiredCommands': ['babeldoc-stream'], 'nativeTarget': 'aarch64-apple-darwin'}], indent=2) + '\n')
print(json.dumps({'packageId': report['packageId'], 'nativeBuildCheckPreparePassed': True, 'sourceFilesMatchFinalArchive': True, 'published': False}), flush=True)
