// Browser proof: the workbench runs in Chromium under Studio's view policy and
// drives the real agent/run.mjs through the aster.code.agent.v1 turn loop.
//
//   node tests/browser.proof.mjs
//
// The host page stands in for Studio: sandboxed iframe, AIX document
// handshake, one warm agent process for every turn, and the model gateway.
// Episode 1 answers each System One model_request with the baseline at 0.8.
// Episode 2 answers 401, so the agent falls back to its rule. Episode 3
// refuses every turn, as Studio does when the agent cannot start, so the
// view's rule controller chooses and the decision panel says the agent is
// unavailable. A last episode answers 0.4, below the 0.55 threshold, and must
// wait for a person.
import { spawn } from 'node:child_process'
import { randomBytes } from 'node:crypto'
import { join } from 'node:path'
import { playwright, root, serveView } from './view-host.mjs'

const { chromium } = playwright
const WIDTH = 1440
const HEIGHT = 900
const token = randomBytes(32).toString('hex')
const documentId = randomBytes(32).toString('hex')
const shots = process.env.PROOF_SCREENSHOTS ?? '/tmp/cobot-arm-proof'

const progress = (message) => process.stderr.write(`proof: ${message}\n`)

function fail(message) {
  throw new Error(message)
}

async function startAgent() {
  const child = spawn(process.execPath, [join(root, 'agent/run.mjs')], { stdio: ['ignore', 'pipe', 'inherit'] })
  process.on('exit', () => child.kill())
  const port = await new Promise((resolve, reject) => {
    child.stdout.once('data', (chunk) => {
      const match = /^port (\d+)/.exec(String(chunk))
      match ? resolve(Number(match[1])) : reject(new Error(`agent said ${chunk}`))
    })
    child.once('exit', (code) => reject(new Error(`agent exited ${code}`)))
  })
  const post = async (path, body) => {
    const response = await fetch(`http://127.0.0.1:${port}${path}`, { method: 'POST', body: JSON.stringify(body) })
    return { status: response.status, body: await response.json() }
  }
  return { child, post }
}

async function main() {
  const agent = await startAgent()
  progress('agent warm')
  const { server, served, origin } = await serveView({ token, documentId, width: WIDTH, height: HEIGHT })
  const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--use-angle=swiftshader'] })
  progress('browser up')
  const page = await browser.newPage({ viewport: { width: WIDTH, height: HEIGHT } })
  const problems = []
  page.on('console', (message) => {
    if (message.type() === 'error' || /Refused|Content Security Policy/.test(message.text())) problems.push(message.text())
  })
  page.on('pageerror', (error) => problems.push(String(error)))

  let mode = 'model'
  let after = 0
  let runs = 0
  const sequences = []
  const turns = []
  await page.exposeFunction('turn', async ({ agent: name, prompt, parameters }) => {
    if (name !== 'cobot-arm') return { reason: 'not_found' }
    const observation = parameters?.observation
    if (!observation?.options || !observation.state?.observation || !observation.decision) fail('turn carries no decision observation')
    if (mode === 'refused') {
      turns.push({ mode, refused: true, keys: Object.keys(observation.options) })
      return { reason: 'agent_failed' }
    }
    const runId = `proof-run-${++runs}`
    const started = performance.now()
    const command = await agent.post('/v1/agent/commands', {
      schema: 'aster.code.agent.v1',
      request_id: `proof-request-${runs}`,
      identity: { session_id: 'proof-session', run_id: runId },
      prompt,
      parameters
    })
    if (command.status !== 200) return { reason: 'agent_failed' }
    let models = 0
    for (let poll = 0; poll < 2000; poll++) {
      const events = (await agent.post('/v1/agent/events:page', { after })).body.events
      for (const event of events) {
        after = event.sequence
        sequences.push(event.sequence)
        if (typeof event.run_id !== 'string' || !event.run_id) fail(`event ${event.sequence} names no run`)
        if (event.run_id !== runId) continue
        if (event.type === 'model_request') {
          models += 1
          if (event.api !== 'systemone' || event.body?.model !== 'platform/apofasi') fail(`model request is not a decision: ${JSON.stringify(event)}`)
          const status = mode === 'rule' ? 401 : 200
          const body =
            status === 200
              ? systemOneReply(event.body, observation.baseline_choice, mode === 'unsure' ? 0.4 : 0.8)
              : { error: 'missing bearer' }
          const answered = await agent.post('/v1/agent/model-responses', { request: event.request, status, body })
          if (answered.status !== 200) fail(`model response refused: ${answered.status}`)
        } else if (event.type === 'output') {
          turns.push({ mode, run: runId, models, ms: Math.round(performance.now() - started), keys: Object.keys(observation.options), ...JSON.parse(event.text) })
          return { text: event.text }
        } else if (event.type === 'failed') {
          return { reason: 'agent_failed' }
        }
      }
      await new Promise((resolve) => setTimeout(resolve, 5))
    }
    return { reason: 'agent_failed' }
  })

  const loadStarted = performance.now()
  await page.goto(`${origin}/host.html`)
  const frame = page.frameLocator('#view')
  const statusText = frame.locator('#status-text')
  try {
    await frame.locator('#loading.hidden').waitFor({ state: 'attached', timeout: 30_000 })
    await frame.locator('#run:not([disabled])').waitFor({ timeout: 10_000 })
  } catch {
    const loading = await frame.locator('#loading').textContent({ timeout: 1000 }).catch(() => '(no frame)')
    fail(`view not ready: loading=${loading} problems=${JSON.stringify(problems)}`)
  }
  const loadMs = Math.round(performance.now() - loadStarted)
  progress(`view ready in ${loadMs} ms`)
  const thresholdLabel = await frame.locator('#threshold-value').textContent()
  const thresholdEnabled = await frame.locator('#threshold').isEnabled()
  await page.waitForTimeout(500)
  await page.screenshot({ path: `${shots}-ready.png` })

  const episodes = []
  for (const [index, current] of ['model', 'rule', 'refused'].entries()) {
    mode = current
    const before = turns.length
    const started = performance.now()
    if (index > 0) {
      await frame.locator('#reset').click()
      await statusText.filter({ hasText: '待命' }).waitFor({ timeout: 10_000 })
    }
    await frame.locator('#run').click()
    try {
      await statusText.filter({ hasText: /验证通过|执行异常|预算耗尽|决策停滞/ }).waitFor({ timeout: 120_000 })
    } catch {
      const status = await statusText.textContent()
      const log = await frame.locator('#events li').allTextContents()
      fail(`episode stuck: status=${status} log=${JSON.stringify(log)} turns=${JSON.stringify(turns.slice(before))} problems=${JSON.stringify(problems)}`)
    }
    const outcome = await statusText.textContent()
    const provider = await frame.locator('#decision-provider').textContent()
    const note = (await frame.locator('#decision-note').isVisible()) ? await frame.locator('#decision-note').textContent() : ''
    const steps = await frame.locator('#events li.event').count()
    progress(`episode ${current}: ${outcome}, ${steps} steps, provider ${provider}`)
    await page.screenshot({ path: `${shots}-${current}.png` })
    const decided = turns.slice(before)
    episodes.push({
      mode: current,
      outcome,
      steps,
      provider,
      note,
      agentTurns: decided.length,
      modelCalls: decided.reduce((sum, turn) => sum + (turn.models ?? 0), 0),
      sources: [...new Set(decided.map((turn) => turn.source ?? 'refused'))],
      wallMs: Math.round(performance.now() - started)
    })
  }

  await frame.locator('#events li.event button').first().click()
  const historyHeading = await frame.locator('#decision-heading').textContent()
  await frame.locator('#decision-live').click()
  await frame.locator('#timeline').fill('40')
  const replayLabel = await frame.locator('#timeline-label').textContent()
  await page.screenshot({ path: `${shots}-replay.png` })
  const canvas = await frame.locator('canvas').boundingBox()

  mode = 'unsure'
  await frame.locator('#reset').click()
  await statusText.filter({ hasText: '待命' }).waitFor({ timeout: 10_000 })
  await frame.locator('#run').click()
  await statusText.filter({ hasText: '等待人工处理' }).waitFor({ timeout: 60_000 })
  const unsure = {
    status: await statusText.textContent(),
    steps: await frame.locator('#events li.event').count(),
    run: await frame.locator('#run').textContent(),
    thresholdEnabled: await frame.locator('#threshold').isEnabled()
  }
  progress(`unsure episode: ${unsure.status}, ${unsure.steps} steps`)
  await page.screenshot({ path: `${shots}-unsure.png` })

  await browser.close()
  server.close()
  const pid = agent.child.pid
  agent.child.kill()

  const increasing = sequences.every((value, index) => index === 0 || value > sequences[index - 1])
  const fonts = [...served].filter((path) => path.endsWith('.woff2')).sort()
  const result = { loadMs, agentPid: pid, agentProcesses: 1, runs, events: sequences.length, sequencesIncrease: increasing, thresholdLabel, thresholdEnabled, episodes, unsure, historyHeading, replayLabel, canvas, fonts, problems }
  console.log(JSON.stringify(result, null, 2))
  if (problems.length) fail('console reported errors or policy violations')
  if (!increasing) fail('event sequence did not increase across runs')
  if (!canvas || canvas.width < 600) fail('the three.js canvas did not fill the viewport')
  if (fonts.length === 0) fail('the view loaded no font')
  const [model, rule, refused] = episodes
  for (const episode of episodes) if (episode.outcome !== '验证通过') fail(`${episode.mode} episode did not succeed`)
  const ofMode = (name) => turns.filter((turn) => turn.mode === name)
  if (!ofMode('model').every((turn) => turn.keys.length > 1 && turn.models === 1 && turn.source === 'model')) {
    fail('a model-episode turn did not use the model reply')
  }
  if (!ofMode('rule').every((turn) => turn.keys.length > 1 && turn.models === 1 && turn.source === 'rule' && turn.note === 'model unavailable (401)')) {
    fail('a 401-episode turn did not fall back to the agent rule')
  }
  if (model.modelCalls === 0 || rule.modelCalls === 0) fail('no multi-option decision reached the agent')
  if (!refused.provider.includes('agent 不可用') || !refused.note.includes('本包 agent 不可用')) fail('the refused episode did not label the rule fallback')
  if (historyHeading !== '第 1 步') fail(`history view heading was ${historyHeading}`)
  if (replayLabel !== '轨迹回放') fail('the timeline did not enter replay')
  if (thresholdLabel !== '0.55' || !thresholdEnabled) fail(`threshold shows ${thresholdLabel}, enabled ${thresholdEnabled}`)
  if (unsure.steps !== 0 || !unsure.thresholdEnabled || !unsure.run.includes('继续')) fail(`the unsure episode did not wait for a person: ${JSON.stringify(unsure)}`)
}

/** The gateway's System One answer: `choice` at `p`, the rest spread over the other offered keys. */
function systemOneReply(body, choice, p) {
  const keys = Object.keys(body.questions.action.criteria)
  const rest = (1 - p) / Math.max(keys.length - 1, 1)
  const probabilities = Object.fromEntries(keys.map((key) => [key, key === choice ? p : rest]))
  return { model: 'apofasi-lexical-0.1.1', answers: { action: { type: 'choice', choice, confidence: p, probabilities } } }
}

setTimeout(() => {
  console.error('proof: timed out')
  process.exit(1)
}, 420_000).unref()

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
