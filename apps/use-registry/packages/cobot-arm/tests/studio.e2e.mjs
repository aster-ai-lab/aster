// Studio end to end: this driver plays Studio's renderer against a real Studio
// authority, over the same routes `aixWorkbench.ts` uses:
//
//   install   PUT  /api/capabilities/installations/aster-cobot-arm  (catalog → bundle → store)
//   list      GET  /api/aix/views
//   open      POST /api/aix/views/sessions
//   load      the view in Chromium from /aix-view/<token>/…  (Studio's policy headers)
//   turns     POST /api/aix/views/sessions/<token>/agents/cobot-arm/turns  (warm aster-box)
//   close     DELETE /api/aix/views/sessions/<token>
//
// It is started by the Studio test `cobot_arm_runs_end_to_end_through_studio`
// and talks to it in lines:
//   stdout  `gateway <origin>`             hermetic mode: the member gateway it serves
//   stdin   `studio <origin> <shell token>`
//   stdout  `result <json>`, then exit 0; any failed check exits non-zero.
//
// Hermetic mode serves the installed set, the bundle and System One, and
// answers each decision with the view's baseline at 0.8, so the episode must
// succeed with every action chosen by the model. Live mode uses the real
// gateway and decision model; it checks the chain, not the model's skill.
import { createHash, randomBytes } from 'node:crypto'
import { readFileSync } from 'node:fs'
import http from 'node:http'
import { createInterface } from 'node:readline'
import { CSP, playwright } from './view-host.mjs'

const MODE = process.env.E2E_MODE === 'live' ? 'live' : 'hermetic'
const SLUG = 'aster-cobot-arm'
const AGENT = 'cobot-arm'
const PACKAGE_ID = 'use/aster/cobot-arm'
// AIX_VIEW_SESSION_PROTOCOL_VERSION in apps/studio/src/shared/types/aixWorkbench.ts.
const VIEW_SESSION_PROTOCOL = 3
const WIDTH = 1440
const HEIGHT = 900
const OUTCOMES = /验证通过|执行异常|预算耗尽|决策停滞|等待人工处理/
const progress = (message) => process.stderr.write(`e2e: ${message}\n`)

function check(condition, message) {
  if (!condition) throw new Error(message)
}

const median = (values) => values.toSorted((a, b) => a - b)[Math.floor(values.length / 2)] ?? null

/** The member gateway in hermetic mode: installed set, bundle, System One. */
function serveGateway(decisions) {
  const bundle = readFileSync(process.env.E2E_BUNDLE)
  const digest = `sha256:${createHash('sha256').update(bundle).digest('hex')}`
  const bearer = `Bearer ${process.env.E2E_MEMBER_TOKEN}`
  const seen = { installs: 0, bundles: 0, decisions: 0, models: new Set(), foreignBearer: 0 }
  let installed = false
  const server = http.createServer((request, response) => {
    const chunks = []
    request.on('data', (chunk) => chunks.push(chunk))
    request.on('end', () => {
      // Studio's member client reads Content-Length bodies, as the gateway sends.
      const send = (status, body, type = 'application/json') => {
        const bytes = type === 'application/json' ? Buffer.from(JSON.stringify(body)) : body
        response.writeHead(status, { 'content-type': type, 'content-length': bytes.length })
        response.end(bytes)
      }
      if (request.headers.authorization !== bearer) {
        seen.foreignBearer += 1
        return send(401, { error: 'unauthorized' })
      }
      const path = new URL(request.url, 'http://127.0.0.1').pathname
      if (request.method === 'GET' && path === '/v1/api/org/capabilities/installations') {
        const entry = { slug: SLUG, kind: 'plugin', installed: true, install: { type: 'plugin', packageType: 'aix', available: true, packageId: PACKAGE_ID, digest, size: bundle.length } }
        return send(200, { data: installed ? [entry] : [] })
      }
      if (request.method === 'PUT' && path === `/v1/api/org/capabilities/installations/${SLUG}`) {
        seen.installs += 1
        installed = true
        return send(200, { data: { slug: SLUG, installed: true } })
      }
      if (request.method === 'GET' && path === `/v1/api/org/capabilities/${SLUG}/bundle`) {
        seen.bundles += 1
        return send(200, bundle, 'application/zip')
      }
      if (request.method === 'POST' && path === '/v1/systemone') {
        const body = JSON.parse(Buffer.concat(chunks).toString('utf8'))
        seen.decisions += 1
        seen.models.add(body.model)
        const keys = Object.keys(body.questions?.action?.criteria ?? {})
        const choice = keys.includes(decisions.baseline) ? decisions.baseline : keys[0]
        const rest = 0.2 / Math.max(keys.length - 1, 1)
        const probabilities = Object.fromEntries(keys.map((key) => [key, key === choice ? (keys.length > 1 ? 0.8 : 1) : rest]))
        return send(200, { model: 'apofasi-e2e', answers: { action: { type: 'choice', choice, confidence: probabilities[choice], probabilities } } })
      }
      send(404, { error: 'not_found' })
    })
  })
  return new Promise((resolve) =>
    server.listen(0, '127.0.0.1', () => resolve({ server, seen, origin: `http://127.0.0.1:${server.address().port}` }))
  )
}

function studioClient(origin, token) {
  return async (method, path, body, { bearer = token } = {}) => {
    const response = await fetch(`${origin}${path}`, {
      method,
      headers: { ...(bearer ? { authorization: `Bearer ${bearer}` } : {}), 'content-type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: AbortSignal.timeout(120_000)
    }).catch((error) => {
      throw new Error(`${method} ${path}: ${error.message}`)
    })
    const text = await response.text()
    let json = null
    try {
      json = JSON.parse(text)
    } catch {}
    return { status: response.status, headers: response.headers, json, text }
  }
}

const identity = () => randomBytes(32).toString('hex')

async function timed(work) {
  const started = performance.now()
  const value = await work()
  return [value, Math.round(performance.now() - started)]
}

async function main() {
  const decisions = { baseline: null }
  const gateway = MODE === 'hermetic' ? await serveGateway(decisions) : null
  if (gateway) console.log(`gateway ${gateway.origin}`)
  const input = createInterface({ input: process.stdin })
  const line = await new Promise((resolve) => input.once('line', resolve))
  input.close()
  const [word, studio, shellToken] = line.split(' ')
  check(word === 'studio' && studio && shellToken, `unexpected line: ${line}`)
  const call = studioClient(studio, shellToken)

  const [install, installMs] = await timed(() => call('PUT', `/api/capabilities/installations/${SLUG}`, {}))
  check(install.status >= 200 && install.status < 300, `install answered ${install.status}: ${install.text}`)
  progress(`installed through the catalog in ${installMs} ms`)

  const [catalogue, catalogueMs] = await timed(() => call('GET', '/api/aix/views'))
  check(catalogue.status === 200, `catalogue answered ${catalogue.status}`)
  const item = catalogue.json.packages?.find((entry) => entry.slug === SLUG)
  check(item, `catalogue lacks ${SLUG}: ${catalogue.text.slice(0, 300)}`)
  const view = item.views?.find((entry) => entry.id === 'workbench')
  check(view, 'the package lists no workbench view')

  const documentId = identity()
  const admission = { requestId: identity(), activationId: identity(), instanceId: documentId, expiresAt: Date.now() + 30_000 }
  const openBody = { protocolVersion: VIEW_SESSION_PROTOCOL, ...admission, slug: item.slug, surface: view.id, revision: item.revision }
  const [opened, openMs] = await timed(() => call('POST', '/api/aix/views/sessions', openBody))
  check(opened.status === 200, `open answered ${opened.status} to ${JSON.stringify(openBody)} for ${JSON.stringify({ ...item, views: undefined })}`)
  const { token, path } = opened.json
  check(/^[a-f0-9]{64}$/.test(token) && path === `/aix-view/${token}/ui/workbench/index.html`, `open returned ${opened.text}`)
  progress(`catalogue ${catalogueMs} ms, open ${openMs} ms`)

  // Studio's own asset route: its policy headers, and nothing outside the view.
  const asset = await fetch(`${studio}${path}`)
  check(asset.status === 200 && asset.headers.get('content-security-policy') === CSP, 'the view entry lacks Studio’s policy')
  check(asset.headers.get('cache-control') === 'no-store' && asset.headers.get('x-content-type-options') === 'nosniff', 'the view entry lacks its cache and type headers')
  const refused = {}
  for (const [name, target] of Object.entries({
    foreignToken: `/aix-view/${identity()}/ui/workbench/index.html`,
    agentSource: `/aix-view/${token}/agent/run.mjs`,
    manifest: `/aix-view/${token}/aster-use-extension.acl`,
    traversal: `/aix-view/${token}/ui/workbench/%2e%2e/%2e%2e/agent/run.mjs`
  })) {
    refused[name] = (await fetch(`${studio}${target}`)).status
  }
  check(Object.values(refused).every((status) => status === 404), `an asset outside the view was served: ${JSON.stringify(refused)}`)
  const turnPath = `/api/aix/views/sessions/${token}/agents`
  const unbound = {
    otherAgent: (await call('POST', `${turnPath}/lab/turns`, { prompt: 'go' })).status,
    noBearer: (await call('POST', `${turnPath}/${AGENT}/turns`, { prompt: 'go' }, { bearer: '' })).status,
    badBody: (await call('POST', `${turnPath}/${AGENT}/turns`, { prompt: '' })).status
  }
  check(unbound.otherAgent === 404 && unbound.noBearer === 401 && unbound.badBody === 400, `turn refusals: ${JSON.stringify(unbound)}`)

  const browser = await playwright.chromium.launch({ args: ['--enable-unsafe-swiftshader', '--use-angle=swiftshader'] })
  const page = await browser.newPage({ viewport: { width: WIDTH, height: HEIGHT } })
  const problems = []
  page.on('console', (message) => {
    if (message.type() === 'error' || /Refused|Content Security Policy/.test(message.text())) problems.push(message.text())
  })
  page.on('pageerror', (error) => problems.push(String(error)))
  const turns = []
  await page.exposeFunction('turn', async ({ agent, prompt, parameters }) => {
    decisions.baseline = parameters?.observation?.baseline_choice ?? null
    const [reply, ms] = await timed(() => call('POST', `${turnPath}/${encodeURIComponent(agent)}/turns`, { prompt, parameters }))
    let decided = null
    try {
      decided = JSON.parse(reply.json?.text ?? 'null')
    } catch {}
    turns.push({ status: reply.status, ms, source: decided?.source ?? null, probability: decided?.probabilities?.[decided?.choice] ?? null, options: Object.keys(parameters?.observation?.options ?? {}).length })
    return reply.status === 200 ? { text: reply.json.text } : { reason: reply.json?.error ?? 'agent_failed' }
  })
  // The same host the renderer is: a sandboxed iframe on Studio's origin and
  // the AIX document handshake on a MessageChannel.
  await page.setContent(`<!doctype html><meta charset="utf-8"><body style="margin:0">
<iframe id="view" sandbox="allow-scripts" style="width:${WIDTH}px;height:${HEIGHT}px;border:0" src="${studio}${path}#aix-document=${documentId}"></iframe>
<script>
addEventListener('message', (event) => {
  const data = event.data
  if (!data || data.type !== 'aix.document.ready' || data.documentId !== '${documentId}') return
  const port = event.ports[0]
  port.onmessage = async ({ data }) => {
    if (data.type !== 'aix.agent.invoke') return
    const reply = await window.turn({ agent: data.agent, prompt: data.prompt, parameters: data.parameters })
    port.postMessage(reply.text === undefined
      ? { protocolVersion: 1, type: 'aix.agent.refused', id: data.id, reason: reply.reason }
      : { protocolVersion: 1, type: 'aix.agent.result', id: data.id, text: reply.text })
  }
  port.start()
  port.postMessage({ protocolVersion: 1, type: 'aix.view.connect', stateJSON: null,
    environment: { theme: 'light', locale: 'zh-CN', direction: 'ltr', tokens: {} } })
})
</script>`)
  const loadStarted = performance.now()
  const frameLocator = page.frameLocator('#view')
  await frameLocator.locator('#loading.hidden').waitFor({ state: 'attached', timeout: 60_000 })
  const frame = page.frames().find((candidate) => candidate.url().includes('/ui/workbench/index.html'))
  await frame.waitForFunction(() => performance.getEntriesByName('cobot:drawn').length > 0, null, { timeout: 30_000 }).catch(async (error) => {
    const seen = await frame.evaluate(() => performance.getEntriesByType('mark').map((mark) => mark.name)).catch(() => [])
    throw new Error(`the view never drew; marks ${JSON.stringify(seen)}; problems ${JSON.stringify(problems)}`, { cause: error })
  })
  const viewWallMs = Math.round(performance.now() - loadStarted)
  const marks = await frame.evaluate(() =>
    Object.fromEntries(performance.getEntriesByType('mark').filter((mark) => mark.name.startsWith('cobot:')).map((mark) => [mark.name.slice(6), Math.round(mark.startTime)]))
  )
  const slowestAssets = (
    await frame.evaluate(() =>
      performance.getEntriesByType('resource').map((entry) => ({ name: entry.name.split('/ui/workbench/')[1] ?? entry.name, start: Math.round(entry.startTime), ms: Math.round(entry.duration) }))
    )
  )
    .toSorted((a, b) => b.ms - a.ms)
    .slice(0, 4)
  progress(`view drawn ${marks.drawn} ms after navigation; slowest assets ${JSON.stringify(slowestAssets)}`)

  const statusText = frameLocator.locator('#status-text')
  const [, episodeMs] = await timed(async () => {
    await frameLocator.locator('#run:not([disabled])').click()
    await statusText.filter({ hasText: OUTCOMES }).waitFor({ timeout: 240_000 })
  })
  const outcome = await statusText.textContent()
  const steps = await frameLocator.locator('#events li.event').count()
  const provider = await frameLocator.locator('#decision-provider').textContent()
  const threshold = Number(await frameLocator.locator('#threshold').inputValue())
  progress(`episode: ${outcome}, ${steps} steps, ${turns.length} turns, ${episodeMs} ms`)
  await browser.close()
  progress('browser closed')

  const [closed, closeMs] = await timed(() => call('DELETE', `/api/aix/views/sessions/${token}`))
  progress(`closed the view: ${closed.status} in ${closeMs} ms`)
  const afterClose = {
    close: closed.status,
    turn: (await call('POST', `${turnPath}/${AGENT}/turns`, { prompt: 'go', parameters: {} })).status,
    asset: (await fetch(`${studio}${path}`, { signal: AbortSignal.timeout(20_000) })).status
  }
  progress(`after close: ${JSON.stringify(afterClose)}`)

  const modelTurns = turns.filter((turn) => turn.source === 'model')
  const result = {
    mode: MODE,
    load: {
      installMs,
      catalogueMs,
      openMs,
      viewWallMs,
      phases: {
        scripts: marks.boot,
        assets: marks.assets - marks.boot,
        world: marks.world - marks.assets,
        scene: marks.ready - marks.world,
        frame: marks.drawn - marks.ready
      },
      drawnMs: marks.drawn,
      slowestAssets
    },
    episode: {
      outcome,
      steps,
      provider,
      episodeMs,
      turns: turns.length,
      modelTurns: modelTurns.length,
      sources: [...new Set(turns.map((turn) => turn.source))],
      threshold,
      probabilities: turns.map((turn) => turn.probability),
      firstTurnMs: turns[0]?.ms ?? null,
      laterTurnMedianMs: median(turns.slice(1).map((turn) => turn.ms))
    },
    gateway: gateway
      ? { installs: gateway.seen.installs, bundles: gateway.seen.bundles, decisions: gateway.seen.decisions, models: [...gateway.seen.models], foreignBearer: gateway.seen.foreignBearer }
      : 'live',
    refused,
    unbound,
    afterClose,
    closeMs,
    problems
  }
  gateway?.server.close()
  console.log(`result ${JSON.stringify(result)}`)

  check(problems.length === 0, `the view reported errors: ${JSON.stringify(problems)}`)
  check(turns.length > 0 && turns.every((turn) => turn.status === 200), `a turn was refused: ${JSON.stringify(turns)}`)
  check(modelTurns.length > 0, 'no turn was decided by the model')
  check(afterClose.close === 200 && afterClose.turn === 404 && afterClose.asset === 404, `the closed view still answers: ${JSON.stringify(afterClose)}`)
  // A model choice below the threshold must stop for a person; one at or above it must not.
  const last = turns.at(-1)
  if (last.source === 'model' && typeof last.probability === 'number') {
    check((outcome === '等待人工处理') === last.probability < threshold, `outcome ${outcome} with p=${last.probability} against threshold ${threshold}`)
  }
  if (MODE === 'hermetic') {
    check(outcome === '验证通过', `the hermetic episode ended ${outcome}`)
    check(turns.every((turn) => turn.source === 'model'), `a hermetic turn fell back: ${JSON.stringify(turns.filter((turn) => turn.source !== 'model'))}`)
    const seen = gateway.seen
    check(seen.installs === 1 && seen.bundles >= 1, `catalog traffic: ${JSON.stringify(result.gateway)}`)
    check(seen.decisions >= modelTurns.length && seen.models.size === 1 && seen.models.has('platform/apofasi'), `decision traffic: ${JSON.stringify(result.gateway)}`)
    check(seen.foreignBearer === 0, 'a gateway call came without the member bearer')
  }
}

setTimeout(() => {
  console.error('e2e: timed out')
  process.exit(1)
}, 480_000).unref()

main().then(
  () => process.exit(0),
  (error) => {
    console.error(error)
    process.exit(1)
  }
)
