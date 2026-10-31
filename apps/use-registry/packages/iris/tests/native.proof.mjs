// Verify the installed page's three missions with Studio's real view-bound agent.
// The browser host is G1's fixture; native authorization and model calls are live.
import assert from 'node:assert/strict'
import { createHash, randomBytes } from 'node:crypto'
import { readFile, readdir, mkdir, writeFile } from 'node:fs/promises'
import { dirname, extname, join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'

import { playwright, serveView } from '../../g1-arm/tests/view-host.mjs'

const runtimePath = process.env.IRIS_NATIVE_RUNTIME
assert.ok(runtimePath, 'IRIS_NATIVE_RUNTIME must name the running Studio runtime receipt')
const runtime = JSON.parse(await readFile(runtimePath, 'utf8'))
const viewRoot = fileURLToPath(new URL('..', import.meta.url))
const evidence = process.env.IRIS_NATIVE_EVIDENCE
const receipt = { sourceMatches: {}, turns: [], missions: [], confirmations: 0 }
const id = () => randomBytes(32).toString('hex')
const digest = (bytes) => createHash('sha256').update(bytes).digest('hex')

async function call(method, path, body) {
  const response = await fetch(runtime.apiBaseUrl + path, {
    method,
    headers: { authorization: `Bearer ${runtime.token}`, 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(190_000)
  })
  return { status: response.status, body: await response.json() }
}

async function files(directory) {
  const entries = await readdir(directory, { withFileTypes: true })
  return (await Promise.all(entries.map((entry) => {
    const path = join(directory, entry.name)
    return entry.isDirectory() ? files(path) : [path]
  }))).flat()
}

let view
let host
let browser
try {
  const catalog = await call('GET', '/api/aix/views')
  assert.equal(catalog.status, 200)
  const installed = catalog.body.packages.find((entry) => entry.slug === 'aster-iris')
  assert.ok(installed, 'Iris must already be installed in this Studio session')
  receipt.packageId = installed.packageId
  receipt.generation = installed.generation
  const opened = await call('POST', '/api/aix/views/sessions', {
    protocolVersion: 3, requestId: id(), activationId: id(), instanceId: id(),
    expiresAt: Date.now() + 30_000, slug: 'aster-iris', surface: 'workbench', revision: installed.revision
  })
  assert.equal(opened.status, 200)
  view = opened.body
  const uiRoot = join(viewRoot, 'ui/workbench')
  for (const path of await files(uiRoot)) {
    // Native views expose browser assets only, never README/license documents.
    if (!['.html', '.css', '.js', '.woff2'].includes(extname(path))) continue
    const name = relative(uiRoot, path)
    const response = await fetch(runtime.apiBaseUrl + dirname(view.path) + '/' + name, {
      headers: { authorization: `Bearer ${runtime.token}` }, signal: AbortSignal.timeout(15_000)
    })
    assert.equal(response.status, 200, `Installed asset unavailable: ${name}`)
    receipt.sourceMatches[name] = digest(await readFile(path)) === digest(Buffer.from(await response.arrayBuffer()))
    assert.equal(receipt.sourceMatches[name], true, `Installed asset differs: ${name}`)
  }

  host = await serveView({ token: id(), documentId: id(), width: 1440, height: 900, viewRoot })
  browser = await playwright.chromium.launch({ args: ['--enable-unsafe-swiftshader', '--use-angle=swiftshader'] })
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
  const problems = []
  page.on('pageerror', (error) => problems.push(error.message))
  await page.exposeFunction('turn', async ({ prompt, parameters, images }) => {
    const started = performance.now()
    const response = await call('POST', `/api/aix/views/sessions/${view.token}/agents/iris/turns`, { prompt, parameters, images })
    const result = response.status === 200 ? JSON.parse(response.body.text) : response.body
    receipt.turns.push({ status: response.status, elapsedMs: Math.round(performance.now() - started), result })
    return response.status === 200 ? { text: response.body.text } : { reason: response.body.error }
  })
  await page.goto(`${host.origin}/host.html`)
  const frame = page.frameLocator('#view')
  for (const task of ['空中巡航', '外卖配送', '设施巡检']) {
    await playwright.expect(frame.locator('#run')).toBeEnabled()
    await frame.getByRole('button', { name: task, exact: true }).click()
    await playwright.expect(frame.locator('#scene-task')).toHaveText(task)
    await frame.getByRole('button', { name: '运行实验', exact: true }).click()
    const deadline = Date.now() + 240_000
    let fallbackNote = ''
    while (await frame.locator('#status-text').innerText() !== '验证通过') {
      assert.ok(Date.now() < deadline, `${task} did not complete within the native turn budget`)
      const status = await frame.locator('#status-text').innerText()
      assert.ok(!['执行异常', '预算耗尽', '决策停滞'].includes(status), await frame.locator('#decision-note').innerText())
      if (status === '等待人工处理') {
        receipt.confirmations += 1
        await frame.getByRole('button', { name: '确认并继续', exact: true }).click()
      }
      const note = await frame.locator('#decision-note').innerText()
      if (note.includes('词法排序引擎')) fallbackNote = note
      if (!receipt.firstAction && await frame.locator('#events li.event').count() > 0) {
        receipt.firstAction = await frame.locator('#events li.event').first().innerText()
        receipt.firstAltitude = await frame.locator('#altitude').innerText()
        assert.ok(parseFloat(receipt.firstAltitude) > 0.5, `The first action did not take off: ${receipt.firstAction}`)
      }
      await new Promise((resolve) => setTimeout(resolve, 200))
    }
    const note = fallbackNote || await frame.locator('#decision-note').innerText()
    receipt.missions.push({ task, status: '验证通过', actions: await frame.locator('#events li.event').count(), note })
    if (evidence) {
      await mkdir(evidence, { recursive: true })
      await page.screenshot({ path: join(evidence, `native-${task}.png`) })
    }
    // A completed episode disables Run until a new task/reset creates an episode.
    await frame.getByRole('button', { name: '重置实验', exact: true }).click()
  }
  assert.ok(receipt.turns.length > 0, 'The missions must call the native agent')
  assert.ok(receipt.turns.every((turn) => turn.status === 200 && (
    turn.result.source === 'model' && !turn.result.model?.startsWith('apofasi-lexical-') ||
    turn.result.source === 'rule' && turn.result.note === 'lexical_decision_engine'
  )), 'Native decisions must be a planning result or an explicitly refused lexical engine')
  receipt.decisionMode = receipt.turns.every((turn) => turn.result.source === 'rule') ? 'lexical_refused_rule' : 'planning_model'
  if (receipt.decisionMode === 'lexical_refused_rule') {
    assert.ok(receipt.missions.every((mission) => mission.note.includes('词法排序引擎')), 'Rule fallback must explain the unavailable planning engine')
  }
  assert.deepEqual(problems, [])
  receipt.status = 'passed'
} catch (error) {
  receipt.status = 'failed'
  receipt.error = error.message
  process.exitCode = 1
} finally {
  await browser?.close()
  if (host) await new Promise((resolve) => host.server.close(resolve))
  if (view) receipt.closed = (await call('DELETE', `/api/aix/views/sessions/${view.token}`)).status
  if (evidence) {
    await mkdir(evidence, { recursive: true })
    await writeFile(join(evidence, 'native-tasks.json'), JSON.stringify(receipt, null, 2) + '\n')
  }
  console.log(JSON.stringify(receipt, null, 2))
}
