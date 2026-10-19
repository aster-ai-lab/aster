import { existsSync, readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

import { defineConfig } from '@rsbuild/core'
import { pluginReact } from '@rsbuild/plugin-react'
import tailwindcss from '@tailwindcss/postcss'
import { tanstackRouter } from '@tanstack/router-plugin/rspack'

import { keepWoff2FontSources, studioDedupeAssetsPlugin } from './scripts/dedupeRendererAssets'
import { flowDesignerResolveAlias } from './scripts/flowDesignerAlias'
import { loadStudioPackageVersion, studioRendererPlugin, studioTenantCopy } from './scripts/studioRendererRsbuild'
import { loadStudioTenantBrand } from './scripts/studioTenantBrand'

const projectRoot = dirname(fileURLToPath(import.meta.url))
const rendererRoot = resolve(projectRoot, 'src/renderer')
const brand = loadStudioTenantBrand(projectRoot, process.env.A3S_TENANT_SLUG)
const version = loadStudioPackageVersion(projectRoot)

const URL_QUERY = /[?&]url(?:&|=|$)/
const UI_THEME_IMPORTS = new Set(['@a3s-lab/ui/styles/theme.css'])

function aliasPackageStyles() {
  return {
    postcssPlugin: 'studio-package-style-alias',
    Once(root: { walkAtRules: (name: string, fn: (rule: { params: string }) => void) => void }) {
      root.walkAtRules('import', (rule) => {
        const quoted = rule.params.match(/['"]([^'"]+)['"]/)
        if (!quoted?.[1] || !UI_THEME_IMPORTS.has(quoted[1])) return
        const file = resolve(projectRoot, '../../packages/ui/src/styles/theme.css')
        rule.params = rule.params.replace(quoted[0], JSON.stringify(file))
      })
    }
  }
}
aliasPackageStyles.postcss = true

function dropInvalidArbitraryProperties() {
  return {
    postcssPlugin: 'studio-drop-invalid-arbitrary',
    OnceExit(root: {
      walkRules: (
        fn: (rule: { walkDecls: (fn: (decl: { value: string }) => void) => void; remove: () => void }) => void
      ) => void
    }) {
      root.walkRules((rule) => {
        let invalid = false
        rule.walkDecls((decl) => {
          if (decl.value.includes('[') || decl.value.includes(']')) invalid = true
        })
        if (invalid) rule.remove()
      })
    }
  }
}

function excludeUrlQuery(rules: unknown): void {
  if (!Array.isArray(rules)) return
  for (const rule of rules) {
    if (!rule || typeof rule !== 'object') continue
    const entry = rule as { test?: RegExp; resourceQuery?: unknown; oneOf?: unknown; rules?: unknown }
    const source = entry.test instanceof RegExp ? entry.test.source : ''
    if (/jsx\?|tsx\?|mjs|cjs|\.js/.test(source) && entry.resourceQuery == null) {
      entry.resourceQuery = { not: URL_QUERY }
    }
    excludeUrlQuery(entry.oneOf)
    excludeUrlQuery(entry.rules)
  }
}

function packageRoot(specifier: string, from = resolve(projectRoot, 'package.json')): string {
  let dir = dirname(createRequire(from).resolve(specifier))
  while (true) {
    const manifestPath = resolve(dir, 'package.json')
    if (existsSync(manifestPath)) {
      const manifest = JSON.parse(readFileSync(manifestPath, 'utf8')) as { name?: string }
      if (manifest.name === specifier) return dir
    }
    const parent = dirname(dir)
    if (parent === dir) break
    dir = parent
  }
  throw new Error(`Unable to locate package root for ${specifier}`)
}

// react-i18next keeps the initialized i18n instance in module state. pnpm keeps
// one copy per react-dom peer, and a reinstall can repoint the hoisted link
// while the dev server runs. Modules rebuilt after that load a copy that never
// saw `initReactI18next` and render raw keys. Pin one resolved copy.
const reactI18nextRoot = packageRoot('react-i18next')
const i18nextRoot = packageRoot('i18next', resolve(reactI18nextRoot, 'package.json'))

const codeMirrorViewRoot = packageRoot('@codemirror/view')
const codeMirrorStateRoot = packageRoot('@codemirror/state', resolve(codeMirrorViewRoot, 'package.json'))
const monacoEsm = resolve(packageRoot('monaco-editor'), 'esm/vs')
const shikiRoot = dirname(createRequire(resolve(projectRoot, 'package.json')).resolve('shiki/package.json'))
// Nested grammars stay on the package exports map. A directory alias drops the `.mjs` target.
const shikiNodeModules = resolve(shikiRoot, '..')

const alias = {
  // The UI kit resolves through a symlink into the repo package, which otherwise
  // loads a second @codemirror/state. Extension objects from the other copy fail
  // `instanceof` and crash the window.
  '@codemirror/state': codeMirrorStateRoot,
  '@codemirror/view': codeMirrorViewRoot,
  '@codemirror/language': packageRoot('@codemirror/language'),
  '@codemirror/lint': packageRoot('@codemirror/lint'),
  '@uiw/react-codemirror': packageRoot('@uiw/react-codemirror'),
  '@uiw/codemirror-extensions-langs': packageRoot('@uiw/codemirror-extensions-langs'),
  '@uiw/codemirror-themes-all': packageRoot('@uiw/codemirror-themes-all'),
  '@renderer': rendererRoot,
  '@shared': resolve(projectRoot, 'src/shared'),
  '@logger': resolve(rendererRoot, 'services/LoggerService'),
  '@data': resolve(rendererRoot, 'data'),
  '@cherrystudio/ai-core/provider': resolve(projectRoot, 'packages/aiCore/src/core/providers'),
  '@cherrystudio/ai-core/built-in/plugins': resolve(projectRoot, 'packages/aiCore/src/core/plugins/built-in'),
  '@cherrystudio/ai-core': resolve(projectRoot, 'packages/aiCore/src'),
  '@cherrystudio/extension-table-plus': resolve(projectRoot, 'packages/extension-table-plus/src'),
  '@cherrystudio/ai-sdk-provider': resolve(projectRoot, 'packages/ai-sdk-provider/src'),
  '@cherrystudio/provider-registry/node': resolve(projectRoot, 'packages/provider-registry/src/registry-loader'),
  '@cherrystudio/provider-registry': resolve(projectRoot, 'packages/provider-registry/src'),
  '@a3s-lab/office/react': resolve(projectRoot, '../../packages/office/dist/react.js'),
  '@a3s-lab/office/core': resolve(projectRoot, '../../packages/office/dist/core.js'),
  '@a3s-lab/office/styles.css': resolve(projectRoot, '../../packages/office/dist/styles.css'),
  '@a3s-lab/office': resolve(projectRoot, '../../packages/office/dist/index.js'),
  '@a3s-lab/intelligent-ui/openui': resolve(projectRoot, '../../packages/intelligent-ui/src/openui.ts'),
  '@a3s-lab/intelligent-ui': resolve(projectRoot, '../../packages/intelligent-ui/src/IntelligentUI.tsx'),
  // Console Flow UI designer. Longer specifiers are listed before shorter prefixes.
  ...flowDesignerResolveAlias(projectRoot),
  '@a3s-lab/ui/icons/providers': resolve(projectRoot, 'packages/ui/src/components/icons/providers'),
  '@a3s-lab/ui/icons': resolve(projectRoot, 'packages/ui/src/components/icons'),
  '@a3s-lab/ui': resolve(projectRoot, 'packages/ui/src'),
  ahooks: resolve(projectRoot, '../../packages/ui/node_modules/ahooks'),
  'react-router-dom': resolve(projectRoot, 'src/renderer/shims/react-router-dom.tsx'),
  // The shared controls live outside this package. Keep one React.
  react$: resolve(projectRoot, 'node_modules/react'),
  'react-dom$': resolve(projectRoot, 'node_modules/react-dom'),
  'react-i18next$': reactI18nextRoot,
  i18next$: i18nextRoot,
  'react/jsx-runtime$': resolve(projectRoot, 'node_modules/react/jsx-runtime.js'),
  'react/jsx-dev-runtime$': resolve(projectRoot, 'node_modules/react/jsx-dev-runtime.js'),
  'monaco-esm': monacoEsm,
  shiki$: shikiRoot
}

export default defineConfig({
  plugins: [pluginReact(), studioRendererPlugin(projectRoot)],
  source: {
    // Office and use packages import ACL with ?raw. It is not a script.
    assetsInclude: /\.acl$/,
    tsconfigPath: './tsconfig.web.json',
    entry: {
      index: './src/renderer/windows/main/entryPoint.tsx'
    },
    define: {
      __APP_EDITION__: JSON.stringify('global'),
      __APP_RELEASE_HISTORY__: JSON.stringify([]),
      __APP_RELEASE_NOTES__: JSON.stringify(''),
      __APP_RELEASE_VERSION__: JSON.stringify(version),
      'import.meta.env.RENDERER_VITE_PPIO_APP_SECRET': JSON.stringify(process.env.RENDERER_VITE_PPIO_APP_SECRET ?? ''),
      'import.meta.env.RENDERER_VITE_AIHUBMIX_SECRET': JSON.stringify(process.env.RENDERER_VITE_AIHUBMIX_SECRET ?? '')
    }
  },
  resolve: {
    alias,
    aliasStrategy: 'prefer-alias'
  },
  html: {
    template: './src/renderer/windows/main/index.html',
    title: brand?.product ?? 'Cherry Studio',
    inject: 'head',
    mountId: 'root',
    meta: {
      charset: false,
      viewport: false
    }
  },
  output: {
    distPath: {
      root: process.env.STUDIO_RENDERER_DIST || '.studio-renderer',
      html: 'windows/main'
    },
    filename: {
      html: 'index.html'
    },
    assetPrefix: '/',
    cleanDistPath: true,
    copy: [
      ...(studioTenantCopy(brand) ?? []),
      { from: resolve(projectRoot, '../../packages/office/dist/pdfium.wasm'), to: 'vendor/embedpdf/pdfium.wasm' }
    ],
    legalComments: 'none',
    sourceMap: false
  },
  splitChunks: {
    preset: 'default',
    minSize: 20000
  },
  performance: {
    printFileSize: false,
    ...(process.env.STUDIO_RENDERER_DIST
      ? { buildCache: { cacheDirectory: `${process.env.STUDIO_RENDERER_DIST}-cache` } }
      : {})
  },
  dev: {
    lazyCompilation: false,
    progressBar: false
  },
  server: {
    host: '127.0.0.1',
    port: 4318,
    strictPort: true,
    open: false,
    historyApiFallback: true,
    headers: {
      'Cache-Control': 'no-store'
    },
    // Preview writes windows/main/index.html. Dev serves the page at /windows/main/.
    // The shell and Tauri both open the preview path, including its query string.
    setup: ({ server }) => {
      server.middlewares.use((req, _res, next) => {
        const raw = req.url ?? ''
        const split = raw.indexOf('?')
        const path = split === -1 ? raw : raw.slice(0, split)
        const query = split === -1 ? '' : raw.slice(split)
        if (path === '/windows/main/index.html') req.url = `/windows/main/${query}`
        next()
      })
    }
  },
  tools: {
    postcss: (config) => {
      config.postcssOptions ??= {}
      const plugins = config.postcssOptions.plugins
      const list = Array.isArray(plugins) ? plugins : []
      list.unshift(aliasPackageStyles)
      // Tailwind's own Lightning CSS minify emits a single line that this
      // PostCSS cannot reparse. Rsbuild minifies CSS afterwards.
      list.push(tailwindcss({ optimize: false }))
      list.push(dropInvalidArbitraryProperties())
      list.push(keepWoff2FontSources())
      config.postcssOptions.plugins = list
    },
    rspack: (config, { rspack }) => {
      const previousIgnored = config.watchOptions?.ignored
      const ignored = [
        '**/src-tauri/**',
        '**/.studio-renderer/**',
        ...(Array.isArray(previousIgnored) ? previousIgnored : previousIgnored ? [previousIgnored] : [])
      ]
      config.watchOptions = { ...config.watchOptions, ignored }
      const nodeShims = resolve(projectRoot, 'scripts/browserNode')
      const shim = (name: string) => resolve(nodeShims, name)
      config.resolve ??= {}
      const modules = config.resolve.modules
      config.resolve.modules = [...(Array.isArray(modules) ? modules : ['node_modules']), shikiNodeModules]
      config.resolve.fallback = {
        ...config.resolve.fallback,
        path: shim('path.cjs'),
        os: shim('os.cjs'),
        crypto: shim('crypto.cjs'),
        url: shim('url.cjs'),
        fs: shim('empty.cjs'),
        stream: shim('empty.cjs'),
        buffer: shim('empty.cjs'),
        util: shim('empty.cjs'),
        http: shim('empty.cjs'),
        https: shim('empty.cjs'),
        zlib: shim('empty.cjs'),
        assert: shim('empty.cjs'),
        constants: shim('empty.cjs'),
        child_process: shim('empty.cjs'),
        events: shim('empty.cjs'),
        'fs/promises': shim('empty.cjs'),
        net: shim('empty.cjs'),
        tls: shim('empty.cjs'),
        tty: shim('empty.cjs'),
        module: shim('empty.cjs'),
        worker_threads: shim('empty.cjs')
      }
      const nodeAlias = {
        'node:path': shim('path.cjs'),
        'node:os': shim('os.cjs'),
        'node:crypto': shim('crypto.cjs'),
        'node:url': shim('url.cjs'),
        'node:fs': shim('empty.cjs'),
        'node:stream': shim('empty.cjs'),
        'node:buffer': shim('empty.cjs'),
        'node:util': shim('empty.cjs'),
        'node:http': shim('empty.cjs'),
        'node:https': shim('empty.cjs'),
        'node:zlib': shim('empty.cjs')
      }
      config.resolve.alias = Array.isArray(config.resolve.alias)
        ? [
            ...config.resolve.alias,
            ...Object.entries(nodeAlias).map(([name, aliasPath]) => ({ name, alias: aliasPath }))
          ]
        : { ...config.resolve.alias, ...nodeAlias }
      config.module ??= {}
      config.module.rules ??= []
      excludeUrlQuery(config.module.rules)
      config.module.rules.unshift({
        resourceQuery: /[?&]url(?:&|=|$)/,
        type: 'asset/resource'
      })
      config.optimization ??= {}
      const splitChunks = config.optimization.splitChunks
      config.optimization.splitChunks =
        splitChunks && typeof splitChunks === 'object' ? { ...splitChunks, minSize: 20000 } : { minSize: 20000 }
      config.plugins ??= []
      config.plugins.push(studioDedupeAssetsPlugin(rspack))
      const studioShikiEntry = resolve(rendererRoot, 'utils/shikiEntry.ts')
      config.plugins.push(
        new rspack.NormalModuleReplacementPlugin(/^shiki$/, (resource: { request: string }) => {
          resource.request = studioShikiEntry
        })
      )
      config.plugins.push(
        new rspack.NormalModuleReplacementPlugin(/^node:/, (resource: { request: string }) => {
          resource.request = resource.request.replace(/^node:/, '')
        })
      )
      config.plugins.push(
        tanstackRouter({
          target: 'react',
          autoCodeSplitting: true,
          routesDirectory: resolve(rendererRoot, 'routes'),
          generatedRouteTree: resolve(rendererRoot, 'routeTree.gen.ts')
        })
      )
      return config
    }
  }
})
