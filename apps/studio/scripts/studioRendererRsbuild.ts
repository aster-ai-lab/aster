import { createReadStream, existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import type { IncomingMessage, ServerResponse } from 'node:http'
import { createRequire } from 'node:module'
import { dirname, relative, resolve, sep } from 'node:path'

import type { RsbuildPlugin } from '@rsbuild/core'

import { studioPreferenceDefaultsJson } from './studioPreferenceDefaults'
import { loadStudioTenantBrand, stampTenantHtml, type StudioTenantBrand } from './studioTenantBrand'
import { isUiSourceFile, windowNameFromHtml } from './uiContract/scan'
import { normalizeSourceFile } from './uiContract/semanticId'
import { transformHtml, transformJsx, UI_CONTRACT_RUNTIME_MODULE_ID } from './uiContract/transform'

const require = createRequire(import.meta.url)

const SEARCH_GLOB = "import.meta.glob('../**/*.search.ts', { eager: true })"
const ENTRY_SCRIPT = '<script type="module" src="/windows/main/entryPoint.tsx"></script>'

const ASSET_TYPES: Record<string, string> = {
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.json': 'application/json',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.webp': 'image/webp',
  '.woff2': 'font/woff2'
}

export function loadStudioPackageVersion(projectRoot: string): string {
  const packageJson = require(resolve(projectRoot, 'package.json')) as { version?: string }
  return packageJson.version ?? '0.0.0'
}

export function studioRendererPlugin(projectRoot: string): RsbuildPlugin {
  const rendererRoot = resolve(projectRoot, 'src/renderer')
  const brand = loadStudioTenantBrand(projectRoot, process.env.A3S_TENANT_SLUG)
  const tenantFiles = brand
    ? new Map<string, string>([
        ['/tenant-icon.png', brand.iconPath],
        ['/tenant-mark.png', brand.markPath],
        ['/tenant-logo.png', brand.logoPath],
        ...(brand.modelMarketPath ? [['/tenant-model-market.json', brand.modelMarketPath] as const] : [])
      ])
    : null
  let preferenceDefaults = '{}'
  try {
    preferenceDefaults = studioPreferenceDefaultsJson(projectRoot)
  } catch (error) {
    console.warn('[studio] preference defaults unavailable; settings will fetch each key', error)
  }
  const bridgePath = resolve(rendererRoot, 'shell/studioMainBridge.js')
  const runtimeEntry = resolve(projectRoot, 'scripts/uiContract/runtimeEntry.ts')

  return {
    name: 'studio-renderer',
    setup(api) {
      api.resolve(({ resolveData }) => {
        if (resolveData.request === UI_CONTRACT_RUNTIME_MODULE_ID) {
          resolveData.request = runtimeEntry
        }
      })

      api.transform({ test: /settingsSearch\/aggregate\.ts$/, order: 'pre' }, ({ code, resourcePath }) => {
        return rewriteSettingsSearchGlob(code, resourcePath)
      })

      api.transform({ test: /\.(jsx|tsx)$/, order: 'pre' }, ({ code, resourcePath }) => {
        if (!isUiSourceFile(resourcePath)) return code
        const sourceFile = normalizeSourceFile(projectRoot, resourcePath)
        if (sourceFile.startsWith('../')) return code
        const result = transformJsx(code, { injectDataUi: true, sourceFile })
        return result.map ? { code: result.code, map: result.map.toString() } : result.code
      })

      api.modifyHTML((html) => {
        const sourceFile = 'src/renderer/windows/main/index.html'
        const contracted = transformHtml(html, {
          injectDataUi: true,
          sourceFile,
          windowName: windowNameFromHtml(sourceFile)
        }).code
        const branded = brand ? stampTenantHtml(contracted, brand) : contracted
        const withoutViteEntry = branded.replace(ENTRY_SCRIPT, '')
        if (withoutViteEntry.includes('__A3S_CHERRY_MAIN__')) return withoutViteEntry
        const appVersion = JSON.stringify(loadStudioPackageVersion(projectRoot))
        const bridgeSource = readFileSync(bridgePath, 'utf8')
        const bridge = `<script>\nglobalThis.__A3S_APP_VERSION__=${appVersion};\nglobalThis.__A3S_PREFERENCE_DEFAULTS__=${preferenceDefaults};\n${bridgeSource}\n</script>`
        return withoutViteEntry.replace('<head>', `<head>\n${bridge}`)
      })

      api.modifyRsbuildConfig((config) => {
        config.server ??= {}
        const previous = config.server.setup
        config.server.setup = async (context) => {
          if (typeof previous === 'function') await previous(context)
          context.server.middlewares.use((req, res, next) => {
            if (serveTenantFile(tenantFiles, req, res)) return
            if (serveRendererAsset(rendererRoot, req, res)) return
            if (context.action === 'dev' && req.url?.split('?')[0] === '/windows/main/index.html') {
              const query = req.url.includes('?') ? `?${req.url.slice(req.url.indexOf('?') + 1)}` : ''
              req.url = `/${query}`
            }
            next()
          })
        }
      })
    }
  }
}

export function studioTenantCopy(brand: StudioTenantBrand | null): { from: string; to: string }[] | undefined {
  if (!brand) return undefined
  return [
    { from: brand.iconPath, to: 'tenant-icon.png' },
    { from: brand.markPath, to: 'tenant-mark.png' },
    { from: brand.logoPath, to: 'tenant-logo.png' },
    ...(brand.modelMarketPath ? [{ from: brand.modelMarketPath, to: 'tenant-model-market.json' }] : [])
  ]
}

function rewriteSettingsSearchGlob(code: string, resourcePath: string): string {
  if (!code.includes(SEARCH_GLOB)) return code
  const settingsRoot = resolve(dirname(resourcePath), '..')
  const files = collectSearchFiles(settingsRoot)
  const imports: string[] = []
  const entries: string[] = []
  files.forEach((file, index) => {
    const spec = relative(dirname(resourcePath), file).replaceAll('\\', '/')
    const keyed = spec.startsWith('.') ? spec : `./${spec}`
    imports.push(`import * as __studioSearch${index} from ${JSON.stringify(keyed.replace(/\.ts$/, ''))}`)
    entries.push(`${JSON.stringify(keyed)}: __studioSearch${index}`)
  })
  return `${imports.join('\n')}\n${code.replace(SEARCH_GLOB, `({${entries.join(',')}})`)}`
}

function collectSearchFiles(directory: string): string[] {
  const files: string[] = []
  for (const name of readdirSync(directory)) {
    if (name === 'node_modules' || name === '__tests__') continue
    const path = resolve(directory, name)
    if (statSync(path).isDirectory()) {
      files.push(...collectSearchFiles(path))
    } else if (name.endsWith('.search.ts')) {
      files.push(path)
    }
  }
  return files.sort()
}

function serveTenantFile(tenantFiles: Map<string, string> | null, req: IncomingMessage, res: ServerResponse): boolean {
  if (!tenantFiles) return false
  const path = req.url?.split('?')[0]
  const file = path ? tenantFiles.get(path) : undefined
  if (!file) return false
  res.setHeader('Content-Type', path?.endsWith('.json') ? 'application/json' : 'image/png')
  res.setHeader('Cache-Control', 'public, max-age=86400')
  createReadStream(file).pipe(res)
  return true
}

function serveRendererAsset(rendererRoot: string, req: IncomingMessage, res: ServerResponse): boolean {
  const urlPath = req.url?.split('?')[0]
  if (!urlPath?.startsWith('/assets/')) return false
  let decoded: string
  try {
    decoded = decodeURIComponent(urlPath.slice(1))
  } catch {
    return false
  }
  if (decoded.includes('\0')) return false
  const assetsRoot = resolve(rendererRoot, 'assets')
  const file = resolve(rendererRoot, decoded)
  if (file !== assetsRoot && !file.startsWith(`${assetsRoot}${sep}`)) return false
  if (!existsSync(file) || !statSync(file).isFile()) return false
  const extension = file.slice(file.lastIndexOf('.'))
  res.setHeader('Content-Type', ASSET_TYPES[extension] ?? 'application/octet-stream')
  res.setHeader('Cache-Control', 'public, max-age=86400')
  createReadStream(file).pipe(res)
  return true
}
