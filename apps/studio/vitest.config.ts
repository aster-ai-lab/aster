import { readdirSync } from 'node:fs'
import { createRequire } from 'node:module'
import process from 'node:process'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'path'

import { tanstackRouter } from '@tanstack/router-plugin/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vitest/config'

import { flowDesignerResolveAlias } from './scripts/flowDesignerAlias'
import { uiContractPlugin } from './scripts/uiContract/vitePlugin'

const studioDir = dirname(fileURLToPath(import.meta.url))
const officeDist = resolve(studioDir, '../../packages/office/dist')
const require = createRequire(import.meta.url)
const monacoEsm = resolve(dirname(dirname(dirname(require.resolve('monaco-editor')))), 'esm/vs')
const shikiNodeModules = resolve(dirname(require.resolve('shiki/package.json')), '..')
const shikiThemeRoot = resolve(shikiNodeModules, '@shikijs/themes/dist')
const shikiThemeAliases = readdirSync(shikiThemeRoot)
  .filter((name) => name.endsWith('.mjs') && name !== 'index.mjs')
  .map((name) => ({
    find: `@shikijs/themes/${name.slice(0, -'.mjs'.length)}`,
    replacement: resolve(shikiThemeRoot, name)
  }))
  .sort((left, right) => right.find.length - left.find.length)

const rendererAlias = {
  ...flowDesignerResolveAlias(studioDir),
  '@a3s-lab/ui/icons/providers': resolve('packages/ui/src/components/icons/providers'),
  '@a3s-lab/ui/icons': resolve('packages/ui/src/components/icons'),
  '@a3s-lab/ui': resolve('packages/ui/src'),
  '@a3s-lab/office/react': resolve(officeDist, 'react.js'),
  '@a3s-lab/office/core': resolve(officeDist, 'core.js'),
  '@a3s-lab/office/styles.css': resolve(officeDist, 'styles.css'),
  '@a3s-lab/office': resolve(officeDist, 'index.js'),
  '@a3s-lab/intelligent-ui/openui': resolve('../../packages/intelligent-ui/src/openui.ts'),
  '@a3s-lab/intelligent-ui': resolve('../../packages/intelligent-ui/src/IntelligentUI.tsx'),
  '@renderer': resolve('src/renderer'),
  '@shared': resolve('src/shared'),
  '@logger': resolve('src/renderer/services/LoggerService'),
  '@data': resolve('src/renderer/data'),
  '@cherrystudio/ai-core/provider': resolve('packages/aiCore/src/core/providers'),
  '@cherrystudio/ai-core/built-in/plugins': resolve('packages/aiCore/src/core/plugins/built-in'),
  '@cherrystudio/ai-core': resolve('packages/aiCore/src'),
  '@cherrystudio/extension-table-plus': resolve('packages/extension-table-plus/src'),
  '@cherrystudio/ai-sdk-provider': resolve('packages/ai-sdk-provider/src'),
  '@cherrystudio/provider-registry/node': resolve('packages/provider-registry/src/registry-loader'),
  '@cherrystudio/provider-registry': resolve('packages/provider-registry/src'),
  ahooks: resolve('../../packages/ui/node_modules/ahooks'),
  'react-router-dom': resolve('src/renderer/shims/react-router-dom.tsx'),
  '@test-mocks': resolve('tests/__mocks__'),
  'monaco-esm': monacoEsm
}

const rendererPlugins = [
  uiContractPlugin(),
  tanstackRouter({
    target: 'react',
    autoCodeSplitting: true,
    routesDirectory: resolve('src/renderer/routes'),
    generatedRouteTree: resolve('src/renderer/routeTree.gen.ts')
  }),
  react()
]

// The repository intentionally remains CommonJS while Vite bundles its TypeScript
// config files. Native config loading cannot parse that combination yet.
process.env.VITE_CONFIG_NATIVE_IGNORE_WARNING = 'true'

// Pin the test timezone to UTC so date-dependent tests are deterministic on every
// machine. CI runners default to UTC; without this, tests that bucket UTC timestamps
// by local day (e.g. Topics "Today/Yesterday") pass in CI but fail on dev machines in
// a non-UTC zone. Set here (main process, before workers spawn) so every thread worker
// inherits TZ=UTC at creation and V8 parses Date in UTC from the start.
process.env.TZ = 'UTC'

// Fork workers pipe their output into the runner streams. The full suite can
// legitimately attach more listeners than Node's default warning threshold.
process.stdout.setMaxListeners(64)
process.stderr.setMaxListeners(64)

export default defineConfig({
  test: {
    projects: [
      // 渲染进程单元测试配置
      {
        extends: true,
        plugins: rendererPlugins,
        server: {
          fs: {
            allow: [resolve(studioDir, '../..')]
          }
        },
        resolve: {
          dedupe: ['react', 'react-dom'],
          alias: [
            { find: /^shiki$/, replacement: resolve(studioDir, 'src/renderer/utils/shikiEntry.ts') },
            ...Object.entries(rendererAlias).map(([find, replacement]) => ({ find, replacement })),
            { find: /^react$/, replacement: resolve('node_modules/react') },
            { find: /^react-dom$/, replacement: resolve('node_modules/react-dom') },
            { find: /^react\/jsx-runtime$/, replacement: resolve('node_modules/react/jsx-runtime.js') },
            { find: /^react\/jsx-dev-runtime$/, replacement: resolve('node_modules/react/jsx-dev-runtime.js') },
            ...shikiThemeAliases
          ]
        },
        test: {
          name: 'renderer',
          environment: 'jsdom',
          // Office dist imports @tiptap/react from packages/office/node_modules.
          // Vitest matches server.deps.inline against the absolute path, so a
          // package-name prefix never hits. Inlining these modules lets the
          // React alias above be the only copy the editor hooks see.
          server: {
            deps: {
              inline: [
                /@tiptap\/react/,
                /crates\/flow\/packages\/ui\/src/,
                /@xyflow\/react/,
                /@phosphor-icons\/react/,
                /packages\/form\/src/,
                /packages\/ce\/dist/,
                /apps\/web\/src\/flow-designer/,
                // Repo-root bun nests React 19.3 beside Studio's 19.2.3. Dialog
                // and its scroll lock then call useRef on the wrong copy.
                // Inline those packages so the React alias above is the only copy.
                /@radix-ui\/react-/,
                /react-day-picker/,
                /@floating-ui\//,
                /framer-motion/,
                /\/motion\//,
                /\/cmdk\//,
                /react-remove-scroll/,
                /react-dropzone/,
                /react-remove-scroll-bar/,
                /react-style-singleton/,
                /use-callback-ref/,
                /use-sidecar/,
                /aria-hidden/
              ]
            }
          },
          setupFiles: ['@vitest/web-worker', 'tests/renderer.setup.ts'],
          include: ['src/renderer/**/*.{test,spec}.{ts,tsx}', 'src/renderer/**/__tests__/**/*.{test,spec}.{ts,tsx}'],
          benchmark: {
            include: ['src/renderer/**/*.bench.{ts,tsx}', 'src/renderer/**/__tests__/**/*.bench.{ts,tsx}']
          }
        }
      },
      // 脚本单元测试配置
      {
        extends: true,
        resolve: {
          alias: rendererAlias
        },
        test: {
          name: 'scripts',
          environment: 'node',
          include: ['scripts/**/*.{test,spec}.{ts,tsx}', 'scripts/**/__tests__/**/*.{test,spec}.{ts,tsx}'],
          benchmark: {
            include: ['scripts/**/*.bench.{ts,tsx}', 'scripts/**/__tests__/**/*.bench.{ts,tsx}']
          }
        }
      },
      // aiCore 包单元测试配置
      {
        extends: 'packages/aiCore/vitest.config.ts',
        test: {
          name: 'aiCore',
          environment: 'node',
          include: [
            'packages/aiCore/**/*.{test,spec}.{ts,tsx}',
            'packages/aiCore/**/__tests__/**/*.{test,spec}.{ts,tsx}'
          ],
          benchmark: {
            include: ['packages/aiCore/**/*.bench.{ts,tsx}', 'packages/aiCore/**/__tests__/**/*.bench.{ts,tsx}']
          }
        }
      },
      // shared 包单元测试配置
      {
        extends: true,
        resolve: {
          alias: {
            '@shared': resolve('src/shared'),
            '@cherrystudio/provider-registry/node': resolve('packages/provider-registry/src/registry-loader'),
            '@cherrystudio/provider-registry': resolve('packages/provider-registry/src')
          }
        },
        test: {
          name: 'shared',
          environment: 'node',
          include: ['src/shared/**/*.{test,spec}.{ts,tsx}', 'src/shared/**/__tests__/**/*.{test,spec}.{ts,tsx}'],
          benchmark: {
            include: ['src/shared/**/*.bench.{ts,tsx}', 'src/shared/**/__tests__/**/*.bench.{ts,tsx}']
          }
        }
      },
      // provider-registry 包单元测试配置
      {
        extends: true,
        resolve: {
          alias: {
            '@shared': resolve('src/shared'),
            '@cherrystudio/provider-registry/node': resolve('packages/provider-registry/src/registry-loader'),
            '@cherrystudio/provider-registry': resolve('packages/provider-registry/src')
          }
        },
        test: {
          name: 'provider-registry',
          environment: 'node',
          include: [
            'packages/provider-registry/**/*.{test,spec}.{ts,tsx}',
            'packages/provider-registry/**/__tests__/**/*.{test,spec}.{ts,tsx}'
          ]
        }
      },
      // @a3s-lab/ui lives in the OS packages/ui tree.
      {
        extends: true,
        resolve: {
          alias: {
            '@a3s-lab/ui': resolve(studioDir, 'packages/ui/src'),
            ahooks: resolve(studioDir, '../../packages/ui/node_modules/ahooks')
          }
        },
        test: {
          name: 'ui',
          environment: 'node',
          include: [
            '../../packages/ui/scripts/**/*.{test,spec}.{ts,tsx}',
            '../../packages/ui/src/**/*.{test,spec}.{ts,tsx}',
            '../../packages/ui/src/**/__tests__/**/*.{test,spec}.{ts,tsx}'
          ]
        }
      }
    ],
    // 全局共享配置
    globals: true,
    setupFiles: [],
    exclude: ['**/node_modules/**', '**/dist/**', '**/out/**', '**/build/**'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html', 'lcov', 'text-summary'],
      exclude: [
        '**/node_modules/**',
        '**/dist/**',
        '**/out/**',
        '**/build/**',
        '**/coverage/**',
        '**/tests/**',
        '**/.yarn/**',
        '**/.cursor/**',
        '**/.vscode/**',
        '**/.github/**',
        '**/.husky/**',
        '**/*.d.ts',
        '**/types/**',
        '**/__tests__/**',
        '**/*.{test,spec}.{ts,tsx}',
        '**/*.config.{js,ts}'
      ]
    },
    testTimeout: 20000,
    pool: 'threads',
    // Vitest 4 uses all available parallelism by default. Cap workers so the
    // full suite does not starve subprocess, worker-thread, and timing tests.
    maxWorkers: '50%'
  }
})
