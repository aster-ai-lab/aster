import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

const studioRoot = resolve(__dirname, '../../../..')

const ELECTRON_PACKAGE =
  /^(electron|electron-.+|@electron\/.+|@electron-toolkit\/.+|@sentry\/electron|@paymoapp\/electron-shutdown-handler)$/

const ELECTRON_IMPORT =
  /(?:\bfrom\s+|\bimport\s*(?:\(\s*)?|\brequire\(\s*)['"](?:electron(?:\/[^'"]*)?|electron-[^'"]*|@electron\/[^'"]*|@electron-toolkit\/[^'"]*|@sentry\/electron(?:\/[^'"]*)?|@paymoapp\/electron-shutdown-handler(?:\/[^'"]*)?)['"]/

function isElectronPackage(name: string): boolean {
  return ELECTRON_PACKAGE.test(name)
}

function walkFiles(dir: string, out: string[]): void {
  if (!existsSync(dir)) return
  for (const entry of readdirSync(dir)) {
    if (entry === 'node_modules' || entry === 'target' || entry === 'dist') continue
    const path = join(dir, entry)
    const info = statSync(path)
    if (info.isDirectory()) walkFiles(path, out)
    else out.push(path)
  }
}

function fileHasElectronImport(path: string): boolean {
  const text = readFileSync(path, 'utf8')
  return ELECTRON_IMPORT.test(text)
}

describe('desktop compile path has no Electron', () => {
  it('does not require an Electron package to install, build, or run', () => {
    const manifest = JSON.parse(readFileSync(resolve(studioRoot, 'package.json'), 'utf8')) as {
      dependencies?: Record<string, string>
      devDependencies?: Record<string, string>
      optionalDependencies?: Record<string, string>
      peerDependencies?: Record<string, string>
      scripts?: Record<string, string>
    }
    const names = [
      ...Object.keys(manifest.dependencies ?? {}),
      ...Object.keys(manifest.devDependencies ?? {}),
      ...Object.keys(manifest.optionalDependencies ?? {}),
      ...Object.keys(manifest.peerDependencies ?? {})
    ]
    expect(names.filter(isElectronPackage)).toEqual([])
    const lockfile = readFileSync(resolve(studioRoot, 'pnpm-lock.yaml'), 'utf8')
    const electronLockKey =
      /^ {2}'?(?:electron(?:@|-)|is-electron@|@electron(?:\/|-))|^ {6}'?(?:electron(?:@|-|:)|is-electron:|@electron(?:\/|-))/m
    expect(lockfile).not.toMatch(electronLockKey)
    const launchScripts = ['dev', 'build', 'start', 'preview']
    for (const name of launchScripts) {
      const script = manifest.scripts?.[name] ?? ''
      expect(script).not.toMatch(/\belectron\b/)
    }
  })

  it('rejects Electron package subpath imports', () => {
    const sentryRenderer = ['@sentry', 'electron', 'renderer'].join('/')
    const sentryMain = ['@sentry', 'electron', 'main'].join('/')
    const samples = [
      `from '${sentryRenderer}'`,
      `from "${sentryMain}"`,
      `import('${sentryRenderer}')`,
      `require('${sentryRenderer}')`,
      `import '${sentryRenderer}'`
    ]
    for (const sample of samples) {
      expect(sample).toMatch(ELECTRON_IMPORT)
    }
    expect(`from '${['@sentry', 'browser'].join('/')}'`).not.toMatch(ELECTRON_IMPORT)
  })

  it('has no electron import on the renderer inputs or the Tauri host', () => {
    const files: string[] = []
    walkFiles(resolve(studioRoot, 'src/renderer'), files)
    walkFiles(resolve(studioRoot, 'src-tauri/src'), files)
    walkFiles(resolve(studioRoot, 'src-tauri/capabilities'), files)
    walkFiles(resolve(studioRoot, 'src-tauri/permissions'), files)
    files.push(resolve(studioRoot, 'rsbuild.config.ts'))
    files.push(resolve(studioRoot, 'scripts/studioRendererRsbuild.ts'))
    files.push(resolve(studioRoot, 'scripts/studio-shell.sh'))
    const hits = files.filter((path) => existsSync(path) && fileHasElectronImport(path))
    expect(hits.map((path) => path.slice(studioRoot.length))).toEqual([])
  })

  it('keeps the Electron main and preload trees off the renderer entry', () => {
    expect(existsSync(resolve(studioRoot, 'src/main'))).toBe(false)
    expect(existsSync(resolve(studioRoot, 'src/preload'))).toBe(false)
    const entry = readFileSync(resolve(studioRoot, 'src/renderer/windows/main/entryPoint.tsx'), 'utf8')
    expect(entry).not.toMatch(/src\/main/)
    expect(entry).not.toMatch(/src\/preload/)
    const config = readFileSync(resolve(studioRoot, 'rsbuild.config.ts'), 'utf8')
    expect(config).toContain('./src/renderer/windows/main/entryPoint.tsx')
    expect(config).not.toMatch(/electron-vite|electron-builder/)
    const cargo = readFileSync(resolve(studioRoot, 'src-tauri/Cargo.toml'), 'utf8')
    expect(cargo).not.toMatch(/^electron\b/m)
    expect(dirname(resolve(studioRoot, 'src-tauri/src/lib.rs'))).toContain('src-tauri')
  })
})
