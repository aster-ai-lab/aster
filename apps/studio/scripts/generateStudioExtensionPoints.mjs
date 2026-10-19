import { spawnSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const studio = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const root = resolve(studio, '../..')
const result = spawnSync(
  'rustup',
  [
    'run',
    '1.97.1',
    'cargo',
    'run',
    '--locked',
    '--offline',
    '--quiet',
    '--manifest-path',
    resolve(root, 'Cargo.toml'),
    '-p',
    'a3s-use-extension',
    '--example',
    'studio_contract'
  ],
  { cwd: root, encoding: 'utf8', maxBuffer: 1024 * 1024 }
)
if (result.status !== 0) throw new Error(result.stderr || String(result.error))
const contracts = JSON.parse(result.stdout)
const source =
  '// Generated from a3s-use-extension; run node scripts/generateStudioExtensionPoints.mjs.\n' +
  'export const STUDIO_EXTENSION_POINT_CONTRACTS = ' +
  JSON.stringify(contracts, null, 2) +
  ' as const\n'
const target = resolve(studio, 'src/shared/types/studioExtensionPoints.generated.ts')
const formatted = spawnSync(resolve(studio, 'node_modules/.bin/oxfmt'), ['--stdin-filepath', target], {
  input: source,
  encoding: 'utf8',
  maxBuffer: 1024 * 1024
})
if (formatted.status !== 0) throw new Error(formatted.stderr || String(formatted.error))
const generated = formatted.stdout
if (process.argv.includes('--check')) {
  if (readFileSync(target, 'utf8') !== generated) throw new Error('Studio extension point projection is stale')
} else writeFileSync(target, generated)
