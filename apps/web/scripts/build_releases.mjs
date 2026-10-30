import { readFile, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import path from 'node:path';

const webRoot = fileURLToPath(new URL('../', import.meta.url));
export const repository = 'https://github.com/aster-ai-lab/aster';
// These artifact names are the existing desktop/CLI publishers' contracts.
export const targets = [
  { key: 'desktop.macos', product: 'desktop', platform: 'macos', arch: 'aarch64', formats: [['aster_', '_aarch64.dmg', 'DMG'], ['Aster-darwin-aarch64-v', '.zip', 'ZIP', 'desktop-stable']] },
  { key: 'desktop.windows', product: 'desktop', platform: 'windows', arch: 'x64', formats: [['aster_', '_x64-setup.exe', 'EXE']] },
  { key: 'desktop.kylin', product: 'desktop', platform: 'kylin', arch: 'x86_64', formats: [['aster_', '_kylin-v10_x86_64.run', 'RUN']] },
  { key: 'cli.macos', product: 'cli', platform: 'macos', arch: 'release archive', formats: [['aster-cli-v', '-macos.tar.gz', 'TAR.GZ']] },
  { key: 'cli.linux', product: 'cli', platform: 'linux', arch: 'release archive', formats: [['aster-cli-v', '-linux.tar.gz', 'TAR.GZ']] },
  { key: 'cli.windows', product: 'cli', platform: 'windows', arch: 'release archive', formats: [['aster-cli-v', '-windows.zip', 'ZIP']] },
];

function versionParts(value) {
  if (!/^\d+\.\d+\.\d+$/.test(value)) return null;
  const parts = value.split('.').map(Number);
  return parts.every(Number.isSafeInteger) ? parts : null;
}

function newer(a, b) {
  for (let i = 0; i < 3; i++) if (a[i] !== b[i]) return a[i] > b[i];
  return false;
}

export function selectDownloads(releases) {
  if (!Array.isArray(releases)) throw new Error('Release metadata must be an array');
  return targets.map((target) => {
    let selected;
    let selectedVersion;
    for (const release of releases) {
      if (release.draft !== false || release.prerelease !== false || !release.published_at) continue;
      for (const [prefix, suffix, format, channel] of target.formats) {
        for (const asset of release.assets) {
          if (!asset.name.startsWith(prefix) || !asset.name.endsWith(suffix)) continue;
          const version = asset.name.slice(prefix.length, -suffix.length);
          const parts = versionParts(version);
          if (!parts) continue;
          const versionTag = `${target.product === 'cli' ? 'aster-cli-v' : 'v'}${version}`;
          if (release.tag_name !== versionTag && (!channel || release.tag_name !== channel)) continue;
          const url = `${repository}/releases/download/${release.tag_name}/${asset.name}`;
          if (asset.state !== 'uploaded' || !(asset.size > 0) || asset.browser_download_url !== url) throw new Error(`Invalid published asset: ${target.key}: ${asset.name}`);
          if (!selected || newer(parts, selectedVersion)) {
            selectedVersion = parts;
            selected = { key: target.key, product: target.product, platform: target.platform, arch: target.arch, version, format, tag: release.tag_name, asset: asset.name, url };
          }
        }
      }
    }
    if (!selected) throw new Error(`No published stable installation asset: ${target.key}`);
    return selected;
  });
}

export async function refreshDownloads({ metadata, root = webRoot } = {}) {
  const pages = metadata ?? JSON.parse(execFileSync('gh', ['api', 'repos/aster-ai-lab/aster/releases', '--paginate', '--slurp'], { encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 }));
  const releases = Array.isArray(pages[0]) ? pages.flat() : pages;
  const downloads = selectDownloads(releases);
  await writeFile(path.join(root, 'content/releases.json'), JSON.stringify({ schema: 'aster.web.downloads.v1', downloads }, null, 2) + '\n');
  return downloads;
}

export async function renderDownloadDocument(source, { root = webRoot } = {}) {
  const snapshot = JSON.parse(await readFile(path.join(root, 'content/releases.json'), 'utf8'));
  if (snapshot.schema !== 'aster.web.downloads.v1' || snapshot.downloads.length !== targets.length || snapshot.downloads.some((item, index) => item.key !== targets[index].key)) throw new Error('Invalid download snapshot target order');
  const values = new Map(snapshot.downloads.flatMap((item) => ['url','version','format'].map((field) => [`download.${item.key}.${field}`, item[field]])));
  if (new Set(snapshot.downloads.filter((item) => item.product === 'cli').map((item) => item.version)).size !== 1) throw new Error('Latest CLI release is missing matching platform archives');
  const template = await readFile(path.join(root, 'content', source), 'utf8');
  const html = template.replace(/\{\{([^{}]+)\}\}/g, (_, key) => {
    if (!values.has(key)) throw new Error(`Unknown download placeholder: ${key}`);
    return values.get(key);
  });
  return html.replace('<!doctype html>', '<!doctype html>\n<!-- Generated from content/' + source + ' and releases.json by Astro. -->');
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const args = process.argv.slice(2);
  let metadataFile;
  for (let index = 0; index < args.length; index++) {
    if (args[index] === '--metadata' && args[index + 1] && !args[index + 1].startsWith('--')) metadataFile = args[++index];
    else if (!['--refresh', '--check'].includes(args[index])) throw new Error('Usage: build_releases.mjs [--refresh] [--check] [--metadata file]');
  }
  if (args.includes('--refresh')) {
    await refreshDownloads({ metadata: metadataFile ? JSON.parse(await readFile(metadataFile, 'utf8')) : undefined });
  }
  for (const source of ['home.html', 'download.html']) await renderDownloadDocument(source);
  const snapshot = JSON.parse(await readFile(path.join(webRoot, 'content/releases.json'), 'utf8'));
  process.stdout.write(JSON.stringify(snapshot.downloads.map(({ key, version, tag }) => ({ key, version, tag }))) + '\n');
}
