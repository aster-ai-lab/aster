import { build } from 'esbuild';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = fileURLToPath(new URL('../', import.meta.url));

export async function buildNavigation({ check = false } = {}) {
  const licenses = await Promise.all(['codehike-MIT.txt', 'diff-BSD-3-Clause.txt'].map(name => readFile(path.join(root, 'licenses', name), 'utf8')));
  const result = await build({
    entryPoints: [path.join(root, 'scripts/docs_navigation.mjs')],
    bundle: true, write: false, platform: 'browser', format: 'iife', target: 'es2022', minify: true,
    legalComments: 'inline', banner: { js: `/* Generated from scripts/docs_navigation.mjs by npm run build.\n\n${licenses.join('\n\n')}\n*/` },
  });
  const output = result.outputFiles[0].text;
  const destination = path.join(root, 'node_modules/.cache/astro-public/docs/navigation.js');
  if (check) {
    if (await readFile(destination, 'utf8') !== output) throw new Error('Stale documentation navigation; run npm --prefix apps/web run build');
  } else {
    await mkdir(path.dirname(destination), { recursive: true });
    await writeFile(destination, output);
  }
}
