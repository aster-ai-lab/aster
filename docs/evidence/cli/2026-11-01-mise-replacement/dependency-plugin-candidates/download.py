from pathlib import Path
import concurrent.futures
import hashlib
import json
import subprocess
import time

root = Path('.tmp/plugin-registry/dependency-upstreams-current')
selected = {
 'uv': ('astral-sh/uv', ['uv-aarch64-apple-darwin.tar.gz', 'uv-x86_64-apple-darwin.tar.gz', 'uv-aarch64-unknown-linux-gnu.tar.gz', 'uv-x86_64-unknown-linux-gnu.tar.gz', 'uv-x86_64-pc-windows-msvc.zip']),
 'bun': ('oven-sh/bun', ['bun-darwin-aarch64.zip', 'bun-darwin-x64-baseline.zip', 'bun-linux-aarch64.zip', 'bun-linux-x64-baseline.zip', 'bun-windows-x64-baseline.zip']),
 'fd': ('sharkdp/fd', ['fd-v10.5.0-aarch64-apple-darwin.tar.gz', 'fd-v10.5.0-x86_64-apple-darwin.tar.gz', 'fd-v10.5.0-aarch64-unknown-linux-gnu.tar.gz', 'fd-v10.5.0-x86_64-unknown-linux-gnu.tar.gz', 'fd-v10.5.0-x86_64-pc-windows-msvc.zip']),
 'rg': ('BurntSushi/ripgrep', ['ripgrep-15.2.0-aarch64-apple-darwin.tar.gz', 'ripgrep-15.2.0-x86_64-apple-darwin.tar.gz', 'ripgrep-15.2.0-aarch64-unknown-linux-gnu.tar.gz', 'ripgrep-15.2.0-x86_64-unknown-linux-musl.tar.gz', 'ripgrep-15.2.0-x86_64-pc-windows-msvc.zip']),
 'rtk': ('rtk-ai/rtk', ['rtk-aarch64-apple-darwin.tar.gz', 'rtk-x86_64-apple-darwin.tar.gz', 'rtk-aarch64-unknown-linux-gnu.tar.gz', 'rtk-x86_64-unknown-linux-musl.tar.gz', 'rtk-x86_64-pc-windows-msvc.zip']),
 'lark-cli': ('larksuite/cli', ['lark-cli-1.0.98-darwin-arm64.tar.gz', 'lark-cli-1.0.98-darwin-amd64.tar.gz', 'lark-cli-1.0.98-linux-arm64.tar.gz', 'lark-cli-1.0.98-linux-amd64.tar.gz', 'lark-cli-1.0.98-windows-amd64.zip']),
 'gh': ('cli/cli', ['gh_2.103.3_macOS_arm64.zip', 'gh_2.103.3_macOS_amd64.zip', 'gh_2.103.3_linux_arm64.tar.gz', 'gh_2.103.3_linux_amd64.tar.gz', 'gh_2.103.3_windows_amd64.zip']),
}
targets = ['darwin-arm64', 'darwin-x86_64', 'linux-arm64', 'linux-x86_64', 'windows-x86_64']
rows = []
for name, (repo, names) in selected.items():
 release = json.loads((root / (repo.replace('/', '--') + '.json')).read_text())['release']
 assert len(names) == len(targets)
 for target, asset_name in zip(targets, names):
  matches = [a for a in release['assets'] if a['name'] == asset_name]
  assert len(matches) == 1, (name, target, asset_name)
  asset = matches[0]
  assert asset['digest'].startswith('sha256:')
  rows.append({'name':name, 'packageId':'aster/' + name, 'repo':repo, 'tag':release['tag_name'], 'target':target, 'asset':asset, 'path':str(root / 'archives' / name / asset_name)})
(root / 'selection.json').write_text(json.dumps(rows, indent=2) + '\n')

def get(row):
 path = Path(row['path'])
 path.parent.mkdir(parents=True, exist_ok=True)
 digest = row['asset']['digest'].removeprefix('sha256:')
 if path.exists() and path.stat().st_size == row['asset']['size'] and hashlib.sha256(path.read_bytes()).hexdigest() == digest:
  return {'name':row['name'], 'target':row['target'], 'passed':True, 'reused':True}
 part = path.with_name(path.name + '.part')
 argv = ['curl', '--fail', '--location', '--silent', '--show-error', '--connect-timeout', '15', '--max-time', '300', '--retry', '2', '--output', str(part), row['asset']['browser_download_url']]
 start = time.monotonic()
 process = subprocess.run(argv, capture_output=True, text=True)
 result = {'name':row['name'], 'target':row['target'], 'argv':argv, 'exitCode':process.returncode, 'stderr':process.stderr, 'seconds':time.monotonic() - start, 'passed':False}
 if process.returncode == 0:
  result['length'] = part.stat().st_size
  result['sha256'] = hashlib.sha256(part.read_bytes()).hexdigest()
  result['passed'] = result['length'] == row['asset']['size'] and result['sha256'] == digest
  if result['passed']:
   part.replace(path)
 return result

results = []
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
 for result in pool.map(get, rows):
  results.append(result)
  (root / 'downloads.json').write_text(json.dumps(results, indent=2) + '\n')
  print(json.dumps({k:result.get(k) for k in ['name','target','passed','exitCode','seconds']}), flush=True)
