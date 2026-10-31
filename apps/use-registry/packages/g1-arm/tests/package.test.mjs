import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

import { wasmBinary } from '../ui/workbench/engine.js'

const root = fileURLToPath(new URL('..', import.meta.url))
const SOURCE_ONLY = ['brand/', 'tests/', 'tools/']
const SHIPPED = [
  'README.md',
  'aster-use-extension.acl',
  'agent/choose.mjs',
  'agent/run.mjs',
  'agents/g1-arm/INSTRUCTIONS.md',
  'skills/g1-arm/SKILL.md',
  'ui/workbench/THIRD_PARTY_NOTICES.md',
  'ui/workbench/app.css',
  'ui/workbench/app.js',
  'ui/workbench/engine.js',
  'ui/workbench/evaluation.js',
  'ui/workbench/fonts/LICENSE',
  'ui/workbench/fonts/ubuntu-latin-400-normal.woff2',
  'ui/workbench/fonts/ubuntu-latin-500-normal.woff2',
  'ui/workbench/fonts/ubuntu-latin-700-normal.woff2',
  'ui/workbench/incremental.js',
  'ui/workbench/index.html',
  'ui/workbench/meshes.js',
  'ui/workbench/perception.js',
  'ui/workbench/planning.js',
  'ui/workbench/robot-scene.js',
  'ui/workbench/session.js',
  'ui/workbench/vendor/lucide/LICENSE',
  'ui/workbench/vendor/lucide/icons.js',
  'ui/workbench/vendor/mujoco/LICENSE',
  'ui/workbench/vendor/mujoco/mujoco.js',
  'ui/workbench/vendor/mujoco/wasm.js',
  'ui/workbench/vendor/g1/LICENSE',
  'ui/workbench/vendor/g1/LICENSE-panda',
  'ui/workbench/vendor/g1/g1.js',
  'ui/workbench/vendor/three/LICENSE',
  'ui/workbench/vendor/three/OrbitControls.js',
  'ui/workbench/vendor/three/three.core.min.js',
  'ui/workbench/vendor/three/three.module.min.js',
  'ui/workbench/world.js'
]
const OWN_MODULES = ['app.js', 'engine.js', 'meshes.js', 'planning.js', 'robot-scene.js', 'session.js', 'world.js'].map(
  (name) => `ui/workbench/${name}`
)
const FILE_MAX_BYTES = 8 * 1024 * 1024

function files(directory) {
  return readdirSync(directory).flatMap((name) => {
    const path = join(directory, name)
    return statSync(path).isDirectory() ? files(path) : [relative(root, path).split('\\').join('/')]
  })
}

const read = (path) => readFileSync(join(root, path), 'utf8')

test('the package ships exactly the declared files, each under the API file limit', () => {
  const shipped = files(root)
    .filter((path) => !path.startsWith('.') && !SOURCE_ONLY.some((prefix) => path.startsWith(prefix)))
    .sort()
  assert.deepEqual(shipped, [...SHIPPED].sort())
  for (const path of SHIPPED) assert.ok(statSync(join(root, path)).size <= FILE_MAX_BYTES, path)
})

test('the manifest declares schema 3 with an edge agent and a contained view', () => {
  const manifest = read('aster-use-extension.acl')
  assert.match(manifest, /^\s*schema_version\s*=\s*3$/m)
  assert.match(manifest, /^\s*harness\s*=\s*"package"$/m)
  assert.match(manifest, /^\s*runtime\s*=\s*"edge"$/m)
  assert.match(manifest, /^\s*entry\s*=\s*"agent\/run\.mjs"$/m)
  assert.match(manifest, /^\s*root\s*=\s*"ui\/workbench"$/m)
  assert.match(manifest, /^\s*scripts\s*=\s*\["ui\/workbench\/app\.js"\]$/m)
  for (const path of manifest.matchAll(/"((?:agent|agents|skills|ui)\/[^"]+)"/g)) {
    assert.ok(SHIPPED.includes(path[1]) || path[1] === 'ui/workbench', path[1])
  }
})

test('the view and agent reach no host service', () => {
  for (const path of SHIPPED.filter((p) => /\.(?:html|css|m?js|acl|md)$/.test(p))) {
    if (path.includes('/vendor/')) continue
    assert.doesNotMatch(read(path), /8097|serve-lab/, path)
    if (path.startsWith('ui/')) assert.doesNotMatch(read(path), /localhost|127\.0\.0\.1/, path)
  }
  const page = read('ui/workbench/index.html') + read('ui/workbench/app.css')
  assert.doesNotMatch(page, /(?:src|href)\s*=\s*"(?:https?:)?\/\//)
  assert.doesNotMatch(page, /url\(\s*['"]?(?:https?:)?\/\//)
  for (const path of OWN_MODULES) {
    assert.doesNotMatch(read(path), /\bfetch\(|XMLHttpRequest|WebSocket|EventSource|new Worker/, path)
  }
  for (const path of SHIPPED.filter((p) => p.startsWith('ui/') && p.endsWith('.js'))) {
    for (const [, target] of read(path).matchAll(/(?:^|[;\n}])\s*(?:import|export)\b[^'"]*?from\s*['"]([^'"]+)['"]/g)) {
      assert.match(target, /^\.\.?\//, `${path} imports ${target}`)
    }
  }
  for (const path of ['agent/run.mjs', 'agent/choose.mjs']) {
    assert.doesNotMatch(read(path), /\bfetch\(|https?:\/\/|Authorization|Bearer|API_KEY/i, path)
  }
})

test('the packed engine restores the upstream MuJoCo 3.15.0 WebAssembly bytes', async () => {
  const bytes = await wasmBinary()
  assert.equal(bytes.length, 10370402)
  assert.equal(
    createHash('sha256').update(bytes).digest('hex'),
    'd59c489dd521c80af6965756c5b90daff549fa281566544e4540c170d937633d'
  )
})

test('the shipped mujoco.js is the recorded patch of upstream and generates no code at run time', () => {
  const script = read('ui/workbench/vendor/mujoco/mujoco.js')
  assert.equal(
    createHash('sha256').update(script).digest('hex'),
    '464aaf6874012368292ee2789de83f1ee4a090b6ebebb68352c6c9d90e374a4c'
  )
  assert.doesNotMatch(script, /new Function|\beval\(/)
  assert.equal(script.split('// aster patch:').length - 1, 2)
})
