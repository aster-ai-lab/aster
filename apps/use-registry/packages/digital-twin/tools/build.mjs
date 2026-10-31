// Build with the repository's installed toolchain and Studio UI dependencies.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { copyFile, mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('..', import.meta.url))
const repository = resolve(root, '../../../..')
const studio = join(repository, 'apps/studio')
const require = createRequire(join(studio, 'package.json'))
const wasmRoot = join(repository, 'crates/digital-twin/wasm')
const out = join(root, 'ui/editor')
const english = JSON.parse(await readFile(join(root, 'src/locales/en-us.json'), 'utf8'))
const chinese = JSON.parse(await readFile(join(root, 'src/locales/zh-cn.json'), 'utf8'))
assert.deepEqual(Object.keys(chinese).sort(), Object.keys(english).sort(), 'Editor locale keys must match')
await mkdir(join(root, '.build/domain'), { recursive: true })
await mkdir(join(out, 'fonts'), { recursive: true })
execFileSync(
  'rustup',
  [
    'run',
    '1.97.1',
    'cargo',
    'build',
    '--manifest-path',
    join(wasmRoot, 'Cargo.toml'),
    '--release',
    '--target',
    'wasm32-unknown-unknown',
    '--locked'
  ],
  { stdio: 'inherit' }
)
execFileSync(
  'wasm-bindgen',
  [
    '--target',
    'web',
    '--out-dir',
    join(root, '.build/domain'),
    '--out-name',
    'scene',
    join(wasmRoot, 'target/wasm32-unknown-unknown/release/aster_digital_twin_wasm.wasm')
  ],
  { stdio: 'inherit' }
)
const binary = await readFile(join(root, '.build/domain/scene_bg.wasm'))
await writeFile(
  join(root, '.build/domain/bytes.mjs'),
  `export const bytes = Uint8Array.from(atob('${binary.toString('base64')}'), char => char.charCodeAt(0))\n`
)
const result = await Bun.build({
  entrypoints: [join(root, 'src/app.jsx')],
  outdir: out,
  naming: 'app.js',
  target: 'browser',
  format: 'esm',
  minify: true,
  metafile: true,
  define: { 'process.env.NODE_ENV': '"production"' },
  plugins: [
    {
      name: 'repository-ui',
      setup(build) {
        build.onResolve({ filter: /^@aster-lab\/ui\/components\// }, ({ path }) => ({
          path: Bun.resolveSync(
            join(repository, 'packages/ui/src/components', path.slice('@aster-ai-lab/ui/components/'.length)),
            repository
          )
        }))
        build.onResolve({ filter: /^lucide-react$/ }, () => ({
          path: join(dirname(require.resolve('lucide-react/package.json')), 'dist/esm/lucide-react.js')
        }))
        build.onResolve({ filter: /^(react(?:\/.*)?|react-dom(?:\/.*)?|i18next)$/ }, ({ path }) => ({
          path: require.resolve(path)
        }))
      }
    }
  ]
})
assert.equal(result.success, true, result.logs.map(String).join('\n'))
const metadata = typeof result.metafile === 'string' ? JSON.parse(result.metafile) : result.metafile
await writeFile(join(root, '.build/bundle-meta.json'), JSON.stringify(metadata))
const bundled = new Map()
for (const input of Object.keys(metadata.inputs)) {
  let directory = dirname(resolve(input))
  while (directory !== dirname(directory)) {
    if (directory.endsWith('/node_modules')) break
    try {
      const pkg = JSON.parse(await readFile(join(directory, 'package.json'), 'utf8'))
      if (directory.includes('/node_modules/')) bundled.set(`${pkg.name}@${pkg.version}`, { directory, pkg })
      break
    } catch {
      directory = dirname(directory)
    }
  }
}
const rustDependencies = JSON.parse(
  execFileSync(
    'rustup',
    [
      'run',
      '1.97.1',
      'cargo',
      'metadata',
      '--manifest-path',
      join(wasmRoot, 'Cargo.toml'),
      '--locked',
      '--offline',
      '--filter-platform',
      'wasm32-unknown-unknown',
      '--format-version',
      '1'
    ],
    { encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 }
  )
)
for (const pkg of rustDependencies.packages)
  if (pkg.source) bundled.set(`Rust ${pkg.name}@${pkg.version}`, { directory: dirname(pkg.manifest_path), pkg })
let notices =
  '# Third-party notices\n\nGenerated from the installed JavaScript bundle dependencies and Rust domain build dependencies.\n'
for (const [name, { directory, pkg }] of [...bundled].sort(([left], [right]) => left.localeCompare(right))) {
  const files = (await readdir(directory)).filter((file) => /^(licen[cs]e|copying|notice)([.-]|$)/i.test(file)).sort()
  notices += `\n## ${name}\n\nLicense: ${pkg.license ?? 'see below'}.\n`
  for (const file of files)
    notices += `\n${file}\n\n\`\`\`text\n${await readFile(join(directory, file), 'utf8')}\n\`\`\`\n`
  if (!files.length)
    notices +=
      '\n```text\n' +
      (await readFile(join(root, 'tools/licenses', name.replaceAll('/', '__') + '.txt'), 'utf8')) +
      '\n```\n'
}
notices +=
  '\n## Three.js 0.180.0\n\nMIT. Bundled from G1’s unmodified Three.js runtime and OrbitControls. TransformControls is the upstream r180 source with its import path changed.\n\n```text\n' +
  (await readFile(join(root, '../g1-arm/ui/workbench/vendor/three/LICENSE'), 'utf8')) +
  '\n```\n'
notices +=
  '\n## Ubuntu font\n\nUnmodified latin weights 400, 500 and 700 from @fontsource/ubuntu 5.3.0. Ubuntu Font Licence 1.0; full text in fonts/LICENSE.\n'
const rustSysroot = execFileSync('rustup', ['run', '1.97.1', 'rustc', '--print', 'sysroot'], {
  encoding: 'utf8'
}).trim()
const rustNotices = join(rustSysroot, 'share/doc/rust')
await mkdir(join(out, 'notices/rust/licenses'), { recursive: true })
await copyFile(join(rustNotices, 'COPYRIGHT-library.html'), join(out, 'notices/rust/COPYRIGHT-library.html'))
for (const file of await readdir(join(rustNotices, 'licenses')))
  await copyFile(join(rustNotices, 'licenses', file), join(out, 'notices/rust/licenses', file))
notices +=
  '\n## Rust 1.97.1 standard library\n\nThe toolchain’s complete standard-library copyright notice and referenced license texts are retained in notices/rust/COPYRIGHT-library.html and notices/rust/licenses/.\n'
await writeFile(join(out, 'THIRD_PARTY_NOTICES.md'), notices)
const tailwindRoot = dirname(require.resolve('tailwindcss/package.json'))
const css =
  `@import "${tailwindRoot}/index.css" source(none);\n@import "${join(repository, 'packages/ui/src/styles/theme.css')}";\n@source "${join(root, 'src')}";\n@source "${join(repository, 'packages/ui/src/components/primitives')}";\n` +
  (await readFile(join(root, 'src/editor.css'), 'utf8'))
const styles = await require('postcss')([require('@tailwindcss/postcss')({ optimize: true })]).process(css, {
  from: join(root, 'src/editor.css')
})
await writeFile(join(out, 'app.css'), styles.css)
for (const name of [
  'ubuntu-latin-400-normal.woff2',
  'ubuntu-latin-500-normal.woff2',
  'ubuntu-latin-700-normal.woff2',
  'LICENSE'
]) {
  await copyFile(join(root, '../g1-arm/ui/workbench/fonts', name), join(out, 'fonts', name))
}
console.log(`Built digital-twin editor; domain WASM ${binary.length} bytes`)
