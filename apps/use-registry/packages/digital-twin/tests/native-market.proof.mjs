// Live API catalogue and native installation/assets; the browser uses Studio's sandbox fixture.
import assert from 'node:assert/strict'
import { createHash, randomBytes } from 'node:crypto'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { playwright, serveView } from '../../g1-arm/tests/view-host.mjs'

assert.ok(process.env.TWIN_NATIVE_RUNTIME, 'TWIN_NATIVE_RUNTIME names an owned, signed-in Studio runtime')
const runtime = JSON.parse(await readFile(process.env.TWIN_NATIVE_RUNTIME, 'utf8'))
const evidence = process.env.TWIN_NATIVE_EVIDENCE
assert.ok(evidence, 'TWIN_NATIVE_EVIDENCE names an evidence directory')
await mkdir(evidence, { recursive: true })
const root = fileURLToPath(new URL('..', import.meta.url))
const report = { assets: {}, workspaceCount: null }
const identity = () => randomBytes(32).toString('hex')
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex')
const unwrap = (value) => (value && typeof value === 'object' && 'data' in value ? value.data : value)
async function call(method, path, body) {
  const response = await fetch(runtime.apiBaseUrl + path, {
    method,
    headers: { authorization: `Bearer ${runtime.token}`, 'content-type': 'application/json' },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    signal: AbortSignal.timeout(30_000)
  })
  return response
}
let view
let host
let browser
let recoveryPath
let recoverySnapshot
try {
  const market = await call(
    'GET',
    '/api/capabilities?kind=plugin&q=%E6%95%B0%E5%AD%97%E5%AD%AA%E7%94%9F&page=1&pageSize=20'
  )
  assert.equal(market.status, 200)
  const page = unwrap(await market.json())
  const entry = page.items.find((item) => item.slug === 'aster-digital-twin')
  assert.ok(entry, 'market must list the digital twin package')
  assert.equal(entry.name, '数字孪生引擎（预览）')
  assert.equal(entry.install.packageType, 'aix')
  assert.equal(entry.install.available, true)
  report.market = { total: page.total, title: entry.name, digest: entry.install.digest, size: entry.install.size }
  const installed = await call('PUT', '/api/capabilities/installations/aster-digital-twin')
  assert.equal(installed.status, 200)
  await installed.arrayBuffer()
  const catalogue = await call('GET', '/api/aix/views')
  assert.equal(catalogue.status, 200)
  const item = (await catalogue.json()).packages.find((item) => item.slug === 'aster-digital-twin')
  assert.ok(item, 'installed native catalogue must expose the editor')
  assert.equal(item.containers[0].location, 'activitybar')
  assert.equal(item.composerModes[0].editor, 'editor')
  report.installed = { packageId: item.packageId, generation: item.generation, revision: item.revision }
  const opened = await call('POST', '/api/aix/views/sessions', {
    protocolVersion: 3,
    requestId: identity(),
    activationId: identity(),
    instanceId: identity(),
    expiresAt: Date.now() + 30_000,
    slug: item.slug,
    surface: 'workbench',
    revision: item.revision
  })
  assert.equal(opened.status, 200)
  view = await opened.json()
  recoveryPath = `/api/aix/views/sessions/${view.token}/resources/drafts/native-market-proof`
  const previousRecovery = await call('GET', recoveryPath)
  assert.equal(previousRecovery.status, 200)
  recoverySnapshot = await previousRecovery.json()
  if (recoverySnapshot.content !== null) {
    const cleared = await call('PUT', recoveryPath, { expectedRevision: recoverySnapshot.revision,
      operationId: crypto.randomUUID(), content: null })
    assert.equal(cleared.status, 200)
    recoverySnapshot = await cleared.json()
  }
  for (const file of [
    'index.html',
    'app.js',
    'app.css',
    'fonts/ubuntu-latin-400-normal.woff2',
    'fonts/ubuntu-latin-500-normal.woff2',
    'fonts/ubuntu-latin-700-normal.woff2'
  ]) {
    const asset = await call('GET', `${dirname(view.path)}/${file}`)
    assert.equal(asset.status, 200, file)
    const actual = Buffer.from(await asset.arrayBuffer())
    assert.equal(digest(actual), digest(await readFile(join(root, 'ui/editor', file))), file)
    report.assets[file] = digest(actual)
  }
  host = await serveView({
    token: 'a'.repeat(64),
    documentId: 'b'.repeat(64),
    width: 1440,
    height: 900,
    viewRoot: root,
    entry: 'ui/editor/index.html'
  })
  browser = await playwright.chromium.launch({ headless: true, args: ['--enable-webgl', '--use-angle=swiftshader'] })
  const screen = await browser.newPage({ viewport: { width: 1440, height: 900 } })
  const errors = []
  screen.on('pageerror', (error) => errors.push(String(error)))
  await screen.exposeFunction('resource', async (request) => {
    if (request.operation === 'read-draft') {
      const response = await call('GET', recoveryPath)
      return response.ok ? { value: await response.json() } : { reason: 'not_found' }
    }
    if (request.operation === 'save-draft') {
      const response = await call('PUT', recoveryPath, { expectedRevision: request.expectedRevision,
        operationId: request.operationId, content: request.content })
      if (!response.ok) return { reason: response.status === 409 ? 'conflict' : 'outcome_unknown' }
      recoverySnapshot = await response.json()
      return { value: recoverySnapshot }
    }
    if (request.operation !== 'workspaces') return { reason: 'invalid_request' }
    const access = await call('GET', `/api/aix/views/sessions/${view.token}/resources/access`)
    if (access.status !== 200 || !(await access.json()).canRead) return { reason: 'not_found' }
    // Workspace metadata belongs to the renderer bridge; this isolated host has no workspace.
    report.workspaceCount = 0
    report.workspaceSource = 'empty browser host fixture; native resource read grant verified'
    return { value: [] }
  })
  await screen.goto(`${host.origin}/host.html`)
  await playwright.expect.poll(() => report.workspaceCount, { timeout: 10_000 }).not.toBeNull()
  const frame = screen.frameLocator('#view')
  await frame.getByRole('heading', { name: '数字孪生编辑器', exact: true }).waitFor()
  await frame.getByRole('button', { name: '添加方体', exact: true }).click()
  await frame.getByLabel('名称', { exact: true }).fill('安装验证方体')
  await frame.getByLabel('名称', { exact: true }).press('Enter')
  await playwright
    .expect(
      frame.getByRole('navigation', { name: '场景物体' }).getByRole('button', { name: '安装验证方体', exact: true })
    )
    .toBeVisible()
  const document = screen.frames().find((candidate) => candidate.url().includes('ui/editor/index.html'))
  await document.waitForFunction(() => performance.getEntriesByName('digital-twin:first-frame').length === 1)
  assert.ok((await frame.locator('.twin-viewport').screenshot()).length > 12_000, 'painted viewport')
  await screen.screenshot({ path: join(evidence, 'installed-editor.png') })
  assert.deepEqual(errors, [])
  await playwright.expect.poll(() => recoverySnapshot.content).toContain('安装验证方体')
  report.editor = 'drawn and editable from byte-verified installed assets'
  report.recovery = 'checkpoint stored through the real native persistent owner'
} finally {
  if (browser) await browser.close()
  if (host) await new Promise((resolve) => host.server.close(resolve))
  if (recoveryPath && recoverySnapshot?.content !== null) {
    const response = await call('GET', recoveryPath)
    if (response.ok) {
      const current = await response.json()
      const cleared = await call('PUT', recoveryPath, { expectedRevision: current.revision,
        operationId: crypto.randomUUID(), content: null })
      report.recoveryCleared = cleared.status
    }
  }
  if (view) report.closed = (await call('DELETE', `/api/aix/views/sessions/${view.token}`)).status
  await writeFile(join(evidence, 'native-market.json'), JSON.stringify(report, null, 2) + '\n')
}
assert.equal(report.closed, 200)
console.log('PASS live market → AIX install → native contributions/assets → sandbox editor → owned view close')
