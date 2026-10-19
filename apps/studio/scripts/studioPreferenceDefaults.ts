import { spawnSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import { dirname, resolve } from 'node:path'
import process from 'node:process'

const require = createRequire(import.meta.url)

function loadBuildSync(
  projectRoot: string
): (options: Record<string, unknown>) => { outputFiles?: { text: string }[] } {
  const pnpm = resolve(projectRoot, 'node_modules/.pnpm')
  const name = readdirSync(pnpm).find((entry) => entry.startsWith('esbuild@'))
  if (!name) throw new Error('esbuild is not installed')
  const esbuildPath = resolve(pnpm, name, 'node_modules/esbuild')
  return require(esbuildPath).buildSync
}

const cache = new Map<string, { mtime: number; json: string }>()

/**
 * Schema defaults for the Tauri shell. The real Electron main process returns
 * every preference from getAll; the shell has no SQLite, so the renderer would
 * otherwise treat each missing key as uncached and fetch it again on the
 * settings page.
 */
export function studioPreferenceDefaultsJson(projectRoot: string): string {
  const schema = resolve(projectRoot, 'src/shared/data/preference/preferenceSchemas.ts')
  const boot = resolve(projectRoot, 'src/shared/data/bootConfig/bootConfigSchemas.ts')
  const mtime = Math.max(statSync(schema).mtimeMs, statSync(boot).mtimeMs)
  const hit = cache.get(projectRoot)
  if (hit && hit.mtime === mtime) return hit.json

  const disk = resolve(projectRoot, '.studio-renderer/preference-defaults.json')
  try {
    if (statSync(disk).mtimeMs >= mtime) {
      const json = readFileSync(disk, 'utf8')
      cache.set(projectRoot, { mtime, json })
      return json
    }
  } catch {
    // The production bundle has not been written yet.
  }

  const source = `
    import { DefaultPreferences } from ${JSON.stringify(schema)}
    import { DefaultBootConfig } from ${JSON.stringify(boot)}
    const merged = { ...DefaultPreferences.default }
    for (const [key, value] of Object.entries(DefaultBootConfig)) {
      if (String(key).startsWith('temp.')) continue
      merged['BootConfig.' + key] = value
    }
    process.stdout.write(JSON.stringify(merged).replace(/</g, '\\\\u003c'))
  `
  const built = loadBuildSync(projectRoot)({
    stdin: {
      contents: source,
      resolveDir: projectRoot,
      sourcefile: 'studio-preference-defaults.ts',
      loader: 'ts'
    },
    bundle: true,
    platform: 'node',
    format: 'cjs',
    write: false,
    logLevel: 'silent',
    alias: {
      '@shared': resolve(projectRoot, 'src/shared')
    }
  })
  const text = built.outputFiles?.[0]?.text
  if (!text) throw new Error('preference defaults bundle was empty')
  const dir = mkdtempSync(resolve(tmpdir(), 'studio-prefs-'))
  const file = resolve(dir, 'defaults.cjs')
  try {
    writeFileSync(file, text)
    const run = spawnSync(process.execPath, [file], { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 })
    if (run.status !== 0) {
      throw new Error((run.stderr || 'preference defaults failed').slice(0, 2000))
    }
    const json = run.stdout
    if (!json.startsWith('{')) throw new Error('preference defaults were not a JSON object')
    try {
      mkdirSync(dirname(disk), { recursive: true })
      writeFileSync(disk, json)
    } catch {
      // The in-memory copy still serves this process.
    }
    cache.set(projectRoot, { mtime, json })
    return json
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}
