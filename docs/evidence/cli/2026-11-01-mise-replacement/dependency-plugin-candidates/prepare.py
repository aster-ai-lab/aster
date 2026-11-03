from pathlib import Path, PurePosixPath
import datetime
import hashlib
import json
import shutil
import struct
import subprocess
import tarfile
import zipfile

root = Path.cwd()
inputs = root / '.tmp/plugin-registry/dependency-upstreams-current'
output = root / '.tmp/plugin-registry/dependency-aix-candidates'
assert not output.exists(), 'candidate output must be new'
rows = json.loads((inputs / 'selection.json').read_text())
downloads = json.loads((inputs / 'downloads.json').read_text())
assert len(rows) == len(downloads) == 35 and all(row['passed'] for row in downloads)
original = json.loads((root / 'docs/evidence/cli/2026-11-01-mise-replacement/desktop-plugin-inventory.json').read_text())
titles = {row['id']: row['displayName'] for row in original['entries'] if 'displayName' in row}
versions = {'uv':'0.13.6', 'bun':'1.4.4', 'fd':'10.5.0', 'rg':'15.2.0', 'rtk':'1.1.0', 'lark-cli':'1.0.98', 'gh':'2.103.3'}
targets = ['darwin-arm64', 'darwin-x86_64', 'linux-arm64', 'linux-x86_64', 'windows-x86_64']
platforms = dict(zip(targets, ['aarch64-macos-', 'x86_64-macos-', 'aarch64-linux-gnu', 'x86_64-linux-gnu', 'x86_64-windows-msvc']))
prefixes = {
 'uv': ['uv-aarch64-apple-darwin', 'uv-x86_64-apple-darwin', 'uv-aarch64-unknown-linux-gnu', 'uv-x86_64-unknown-linux-gnu', ''],
 'bun': ['bun-darwin-aarch64', 'bun-darwin-x64-baseline', 'bun-linux-aarch64', 'bun-linux-x64-baseline', 'bun-windows-x64-baseline'],
 'fd': ['fd-v10.5.0-aarch64-apple-darwin', 'fd-v10.5.0-x86_64-apple-darwin', 'fd-v10.5.0-aarch64-unknown-linux-gnu', 'fd-v10.5.0-x86_64-unknown-linux-gnu', 'fd-v10.5.0-x86_64-pc-windows-msvc'],
 'rg': ['ripgrep-15.2.0-aarch64-apple-darwin', 'ripgrep-15.2.0-x86_64-apple-darwin', 'ripgrep-15.2.0-aarch64-unknown-linux-gnu', 'ripgrep-15.2.0-x86_64-unknown-linux-musl', 'ripgrep-15.2.0-x86_64-pc-windows-msvc'],
 'rtk': ['', '', '', '', ''],
 'lark-cli': ['', '', '', '', ''],
 'gh': ['gh_2.103.3_macOS_arm64', 'gh_2.103.3_macOS_amd64', 'gh_2.103.3_linux_arm64', 'gh_2.103.3_linux_amd64', ''],
}
licenses = {'uv':['LICENSE-APACHE','LICENSE-MIT'], 'bun':['LICENSE.md'], 'fd':['LICENSE-APACHE','LICENSE-MIT'], 'rg':['COPYING','LICENSE-MIT','UNLICENSE'], 'rtk':['LICENSE','LICENSE-ENGINE'], 'lark-cli':['LICENSE'], 'gh':['LICENSE']}
policy = '''plugin_domains {
 schema_version = 1
 domain "operations" { owners = ["aster-ai-lab"] publishers = ["aster"] }
 domain "development" { owners = ["aster-ai-lab"] publishers = ["aster"] }
 domain "productivity" { owners = ["aster-ai-lab"] publishers = ["aster"] }
}
'''
own_license = (root / 'apps/use-registry/LICENSE').read_bytes()
builder = root / '.tmp/plugin-registry/current-plugin-packages-v2/bin/aster'
results = []
native_inputs = []
for row in rows:
 name, target = row['name'], row['target']
 domain = 'productivity' if name == 'lark-cli' else 'development'
 version = versions[name]
 source = output / 'authoring/plugins' / domain / 'aster' / name / 'variants' / target
 source.mkdir(parents=True)
 prefix = prefixes[name][targets.index(target)]
 bin_dir = PurePosixPath(prefix) / 'bin' if name == 'gh' else PurePosixPath(prefix)
 bin_text = '' if str(bin_dir) == '.' else str(bin_dir)
 executable = name + ('.exe' if target.startswith('windows-') else '')
 entry = str(bin_dir / executable)
 commands = {name: entry}
 if name == 'uv':
  commands['uvx'] = str(bin_dir / ('uvx.exe' if target.startswith('windows-') else 'uvx'))
 archive = Path(row['path'])
 expected_digest = row['asset']['digest'].removeprefix('sha256:')
 assert archive.stat().st_size == row['asset']['size']
 assert hashlib.sha256(archive.read_bytes()).hexdigest() == expected_digest
 if archive.suffix == '.zip':
  upstream = zipfile.ZipFile(archive)
  members = {info.filename for info in upstream.infolist() if not info.is_dir()}
  read_member = upstream.read
 else:
  upstream = tarfile.open(archive, 'r:gz')
  members = {member.name for member in upstream.getmembers() if member.isfile()}
  read_member = lambda path: upstream.extractfile(path).read()
 assert all(path in members for path in commands.values()), (name, target, commands)
 static_linux = None
 if name in ['rg','rtk'] and target == 'linux-x86_64':
  elf = read_member(entry)
  assert elf[:6] == b'\x7fELF\x02\x01'
  phoff = struct.unpack_from('<Q', elf, 32)[0]
  phsize, phnum = struct.unpack_from('<HH', elf, 54)
  static_linux = all(struct.unpack_from('<I', elf, phoff + i * phsize)[0] != 3 for i in range(phnum))
  assert static_linux, 'musl archive requires a static ELF before considering a GNU host'
 legal = source / 'upstream-licenses'
 legal.mkdir()
 for filename in licenses[name]:
  path = str(PurePosixPath(prefix) / filename)
  if path in members:
   data = read_member(path)
  else:
   data = (inputs / 'sources' / row['repo'] / filename).read_bytes()
  (legal / filename).write_bytes(data)
 upstream.close()
 manifest = f'''extension "aster/{name}" {{
 schema_version = 3
 version = "{version}"
 route = "{name}"
 requires_use = ">=0.3.0, <0.4.0"
 actions = ["read"]
 repository {{ url = "https://github.com/aster-ai-lab/plugins" }}
 plugin "toolchain" {{ entry = "toolchain.acl" optional = false }}
}}
'''
 (source / 'aster-use-extension.acl').write_text(manifest, encoding='utf-8')
 command_text = '\n'.join(f' command "{command}" {{ path = "{path}" }}' for command,path in commands.items())
 (source / 'toolchain.acl').write_text(f'''toolchain {{
 schema_version = 1
 name = "{name}"
 spec = "github:{row['repo']}"
 version = "{version}"
 executable = "{name}"
 platform = "{platforms[target]}"
 url = "{row['asset']['browser_download_url']}"
 length = {row['asset']['size']}
 sha256 = "{expected_digest}"
 format = "{'zip' if archive.suffix == '.zip' else 'tar.gz'}"
 prefix = ""
 bin_dirs = ["{bin_text}"]
 executable_mode = "0755"
{command_text}
}}
''', encoding='utf-8')
 (source / 'LICENSE').write_bytes(own_license)
 (source / 'README.md').write_text(f'''# {titles[name]}

`aster/{name}` {version} references the exact upstream release archive from
`{row['repo']}` at `{row['tag']}`. It retains the complete archive and its
source-declared command layout. `upstream-licenses/` preserves the upstream
notices; the root MIT license covers this Aster acquisition recipe.
The upstream installer is not executed. This variant targets `{target}`.

This is a candidate, not an official Registry publication. Publication requires
an implementing desktop host, anonymous GHCR access and native qualification
for the declared target. A format check alone does not qualify a native target.
''', encoding='utf-8')
 repository = output / 'repository' / name / target
 staged = repository / 'plugins' / domain / 'aster' / name
 shutil.copytree(source, staged)
 (repository / 'domains.acl').write_text(policy, encoding='utf-8')
 aix = output / 'artifacts' / name / f'{name}-{version}-{target}.aix'
 aix.parent.mkdir(parents=True, exist_ok=True)
 commands_to_run = [
  [str(builder),'--no-auto-update','use','registry','package','build',str(staged),'--output',str(aix),'--json'],
  [str(builder),'--no-auto-update','use','registry','package','check',str(aix),'--json'],
  [str(builder),'--no-auto-update','use','registry','package','prepare',str(aix),'--repository',str(repository),'--domain',domain,'--output',str(aix.with_suffix('.oci.json')),'--json'],
 ]
 checks = []
 for index,argv in enumerate(commands_to_run):
  process = subprocess.run(argv, capture_output=True, text=True, timeout=60)
  log = output / 'checks' / name / f'{target}-{index}.json'
  log.parent.mkdir(parents=True, exist_ok=True)
  log.write_text(json.dumps({'argv':argv,'exitCode':process.returncode,'stdout':process.stdout,'stderr':process.stderr},indent=2)+'\n')
  checks.append(process.returncode)
  if process.returncode:
   break
 result = {'name':name,'packageId':'aster/'+name,'version':version,'domain':domain,'target':target,'entry':entry,'commands':commands,'checks':checks,'passed':checks==[0,0,0],'upstreamArchiveSha256':row['asset']['digest'],'upstreamArchiveLength':row['asset']['size'],'staticLinuxElf':static_linux}
 if aix.exists():
  result['aixArchiveSha256'] = 'sha256:' + hashlib.sha256(aix.read_bytes()).hexdigest()
  with zipfile.ZipFile(aix) as final:
   expected = {str(p.relative_to(source).as_posix()):p.read_bytes() for p in source.rglob('*') if p.is_file()}
   actual = {path:final.read(path) for path in final.namelist() if not path.endswith('/')}
   assert actual == expected, (name,target,'source/archive differs')
  result['sourceFilesMatchFinalArchive'] = True
 if result['passed'] and target == 'darwin-arm64':
  native_inputs.append({'packageId':'aster/'+name,'aix':str(aix.resolve()),'archive':str(archive.resolve()),'version':version,'tool':name,'versionArgs':['--version'],'requiredCommands':list(commands),'nativeTarget':'aarch64-apple-darwin'})
 results.append(result)
 (output / 'validation.json').write_text(json.dumps({'capturedAtUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'results':results,'published':False},indent=2)+'\n')
 print(json.dumps(result), flush=True)
(output / 'native-inputs.json').write_text(json.dumps(native_inputs,indent=2)+'\n')
print(json.dumps({'candidates':len(results),'passed':sum(row['passed'] for row in results),'nativeInputs':len(native_inputs)}),flush=True)
