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
//
// Vision mode sends two camera frames with each turn. The host checks them as
// Studio does, gives the agent only their manifest, and answers each chat
// model_request from a script: two moves then three holds (a stall), and then
// an option the menu did not offer, which must stop the episode.
//
// Last, a batch evaluation runs both default modes over the three tasks with
// two seeds; its report is stored as view state and must survive a reload.
import { spawn } from 'node:child_process'
import { randomBytes } from 'node:crypto'
import { join } from 'node:path'
import { playwright, root, serveView } from './view-host.mjs'

const { chromium } = playwright
const WIDTH = 1440
const HEIGHT = 900
const token = randomBytes(32).toString('hex')
const documentId = randomBytes(32).toString('hex')
const shots = process.env.PROOF_SCREENSHOTS ?? '/tmp/g1-arm-proof'
const VISION_MODEL = 'bailian/deepseek-v4.1-flash'
const VISION_SCRIPT = ['x_pos_10', 'z_pos_10', 'hold', 'hold', 'hold']
// Same bound as MAX_TURN_IMAGE in Studio's aix_episodes.rs.
const IMAGE_BYTES = 256 * 1024

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
  let visionAnswers = 0
  let viewState = null
  const stores = []
  await page.exposeFunction('viewState', (...value) => {
    if (value.length === 0) return viewState
    viewState = value[0]
    stores.push(viewState)
    return true
  })
  await page.exposeFunction('turn', async ({ agent: name, prompt, parameters, images }) => {
    if (name !== 'g1-arm') return { reason: 'not_found' }
    const observation = parameters?.observation
    if (!observation?.options || !observation.state?.observation || !observation.decision) fail('turn carries no decision observation')
    if (mode === 'refused') {
      turns.push({ mode, refused: true, keys: Object.keys(observation.options) })
      return { reason: 'agent_failed' }
    }
    const vision = mode.startsWith('vision')
    if (vision !== (images !== undefined)) fail(`${mode} turn images: ${images === undefined ? 'none' : images.length}`)
    const frames = (images ?? []).map(cameraFrame)
    const runId = `proof-run-${++runs}`
    const started = performance.now()
    const command = await agent.post('/v1/agent/commands', {
      schema: 'aster.code.agent.v1',
      request_id: `proof-request-${runs}`,
      identity: { session_id: 'proof-session', run_id: runId },
      prompt,
      parameters,
      ...(images ? { images: frames.map(({ name, bytes }) => ({ name, media_type: 'image/jpeg', bytes })) } : {})
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
        if (event.type === 'model_request' && vision) {
          models += 1
          const step = visionAnswers++
          const choice = visionChoice(event, frames, observation, mode, step)
          const body = { choices: [{ finish_reason: 'stop', message: { role: 'assistant', content: JSON.stringify({ choice, intent: `proof step ${step + 1}`, visual_evidence: 'two labelled views' }) } }] }
          const answered = await agent.post('/v1/agent/model-responses', { request: event.request, status: 200, body })
          if (answered.status !== 200) fail(`model response refused: ${answered.status}`)
        } else if (event.type === 'model_request') {
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
          turns.push({
            mode,
            run: runId,
            models,
            ms: Math.round(performance.now() - started),
            keys: Object.keys(observation.options),
            ...(vision ? { frames: frames.map(({ name, width, height, bytes }) => ({ name, width, height, bytes })) } : {}),
            ...JSON.parse(event.text)
          })
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

  // RGB-D: the decisions see only what the external and wrist cameras estimate.
  mode = 'rule'
  const cameraEpisodes = []
  await frame.locator('[data-mode="rgbd"]').click()
  await statusText.filter({ hasText: '待命' }).waitFor({ timeout: 10_000 })
  for (const task of ['transfer', 'stack', 'barrier']) {
    const started = performance.now()
    await frame.locator(`[data-task="${task}"]`).click()
    await statusText.filter({ hasText: '待命' }).waitFor({ timeout: 10_000 })
    await frame.locator('#run').click()
    try {
      await statusText.filter({ hasText: /验证通过|执行异常|预算耗尽|决策停滞/ }).waitFor({ timeout: 120_000 })
    } catch {
      fail(`rgbd ${task} stuck: ${await frame.locator('#camera-status').textContent()} problems=${JSON.stringify(problems)}`)
    }
    const camera = await frame.locator('#camera-status').textContent()
    const worst = Number(/本轮最大 ([\d.]+) mm/.exec(camera)?.[1])
    const captures = Number(/第 (\d+) 帧/.exec(camera)?.[1])
    const finalError = Math.max(Number(/方块误差 ([\d.]+) mm/.exec(camera)?.[1]), Number(/目标误差 ([\d.]+) mm/.exec(camera)?.[1]))
    const images = await frame.locator('.camera-views img').evaluateAll((items) => items.map((img) => [img.naturalWidth, img.naturalHeight, img.src.slice(0, 23)]))
    cameraEpisodes.push({
      task,
      outcome: await statusText.textContent(),
      steps: await frame.locator('#events li.event').count(),
      captures,
      finalEstimateErrorMm: finalError,
      worstEstimateErrorMm: worst,
      camera,
      images,
      wallMs: Math.round(performance.now() - started)
    })
    progress(`rgbd ${task}: ${cameraEpisodes.at(-1).outcome}, ${camera}`)
    await page.screenshot({ path: `${shots}-rgbd-${task}.png` })
  }

  // Vision: each turn carries the two camera frames; the model picks one fixed short step.
  const visionEpisodes = []
  await frame.locator('[data-mode="vision"]').click()
  await statusText.filter({ hasText: '待命' }).waitFor({ timeout: 10_000 })
  await frame.locator('[data-task="transfer"]').click()
  await statusText.filter({ hasText: '待命' }).waitFor({ timeout: 10_000 })
  for (const [index, current] of ['vision', 'vision-unoffered'].entries()) {
    mode = current
    visionAnswers = 0
    const before = turns.length
    if (index > 0) {
      await frame.locator('#reset').click()
      await statusText.filter({ hasText: '待命' }).waitFor({ timeout: 10_000 })
    }
    await frame.locator('#run').click()
    try {
      await statusText.filter({ hasText: /验证通过|执行异常|预算耗尽|决策停滞/ }).waitFor({ timeout: 120_000 })
    } catch {
      fail(`${current} stuck: ${await statusText.textContent()} turns=${JSON.stringify(turns.slice(before))} problems=${JSON.stringify(problems)}`)
    }
    const decided = turns.slice(before)
    visionEpisodes.push({
      mode: current,
      outcome: await statusText.textContent(),
      steps: await frame.locator('#events li.event').count(),
      note: (await frame.locator('#decision-note').isVisible()) ? await frame.locator('#decision-note').textContent() : '',
      camera: await frame.locator('#camera-status').textContent(),
      agentTurns: decided.length,
      modelCalls: decided.reduce((sum, turn) => sum + turn.models, 0),
      answers: decided.map((turn) => turn.choice ?? turn.error),
      sources: [...new Set(decided.map((turn) => turn.source ?? 'none'))],
      imageCounts: [...new Set(decided.map((turn) => turn.images))],
      frames: decided.flatMap((turn) => turn.frames)
    })
    progress(`${current}: ${visionEpisodes.at(-1).outcome}, ${decided.length} turns`)
    await page.screenshot({ path: `${shots}-${current}.png` })
  }
  await frame.locator('[data-mode="privileged"]').click()
  await statusText.filter({ hasText: '待命' }).waitFor({ timeout: 10_000 })
  await frame.locator('[data-task="transfer"]').click()
  await statusText.filter({ hasText: '待命' }).waitFor({ timeout: 10_000 })

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

  // Batch evaluation: the default chips, two seeds each; the report is stored and survives a reload.
  mode = 'rule'
  await frame.locator('#eval-block summary').click()
  await frame.locator('#eval-seeds').fill('2')
  const evalLabel = await frame.locator('#eval-run-label').textContent()
  const evalStarted = performance.now()
  await frame.locator('#eval-run').click()
  try {
    await frame.locator('#eval-progress').filter({ hasText: /^已(完成|停止)/ }).waitFor({ timeout: 600_000 })
  } catch {
    fail(`batch stuck: ${await frame.locator('#eval-progress').textContent()} problems=${JSON.stringify(problems)}`)
  }
  const batch = {
    label: evalLabel,
    progress: await frame.locator('#eval-progress').textContent(),
    wallMs: Math.round(performance.now() - evalStarted),
    rows: await frame.locator('.eval-table tbody tr').evaluateAll((rows) => rows.map((row) => [...row.children].map((cell) => cell.textContent))),
    stores: stores.length
  }
  await page.screenshot({ path: `${shots}-batch.png` })
  await frame.locator('#eval-export-open').click()
  batch.exported = JSON.parse(await frame.locator('#eval-json').inputValue())
  batch.exportNote = await frame.locator('#eval-copy-status').textContent()
  await page.screenshot({ path: `${shots}-batch-export.png` })
  await frame.locator('#eval-export-close').click()
  await page.reload()
  await frame.locator('#loading.hidden').waitFor({ state: 'attached', timeout: 30_000 })
  await frame.locator('#eval-block summary').click()
  batch.restored = {
    progress: await frame.locator('#eval-progress').textContent(),
    rows: await frame.locator('.eval-table tbody tr').count(),
    exportEnabled: await frame.locator('#eval-export-open').isEnabled()
  }
  progress(`batch: ${batch.progress}, ${batch.wallMs} ms, restored ${batch.restored.progress}`)

  await browser.close()
  server.close()
  const pid = agent.child.pid
  agent.child.kill()

  const increasing = sequences.every((value, index) => index === 0 || value > sequences[index - 1])
  const fonts = [...served].filter((path) => path.endsWith('.woff2')).sort()
  const result = { loadMs, agentPid: pid, agentProcesses: 1, runs, events: sequences.length, sequencesIncrease: increasing, thresholdLabel, thresholdEnabled, episodes, cameraEpisodes, visionEpisodes: visionEpisodes.map(({ frames, ...episode }) => ({ ...episode, frames: frames.length, frameSizes: [...new Set(frames.map(({ width, height }) => `${width}x${height}`))], largestFrameBytes: Math.max(...frames.map(({ bytes }) => bytes)) })), unsure, batch: { ...batch, exported: batch.exported.summary }, historyHeading, replayLabel, canvas, fonts, problems }
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
  for (const episode of cameraEpisodes) {
    if (episode.outcome !== '验证通过') fail(`rgbd ${episode.task} episode did not succeed: ${episode.camera}`)
    // A held or partly hidden object may be off by about a centimetre, as upstream on MuJoCo's renderer
    // (stack: 8.6 mm while the cube hides the block); the unobstructed final view must be within 2 mm.
    if (!(episode.captures > 1) || !(episode.finalEstimateErrorMm < 2) || !(episode.worstEstimateErrorMm < 15) || episode.steps > 16) {
      fail(`rgbd ${episode.task} estimates drifted: ${episode.camera}, ${episode.steps} steps`)
    }
    if (!episode.images.every(([width, height, src]) => width === 640 && height === 480 && src === 'data:image/jpeg;base64,')) {
      fail(`rgbd ${episode.task} camera images missing: ${JSON.stringify(episode.images)}`)
    }
  }
  const [scripted, unoffered] = visionEpisodes
  if (scripted.outcome !== '决策停滞' || scripted.agentTurns !== VISION_SCRIPT.length || scripted.modelCalls !== VISION_SCRIPT.length) {
    fail(`the scripted vision episode did not stall after its script: ${JSON.stringify(scripted)}`)
  }
  if (JSON.stringify(scripted.answers) !== JSON.stringify(VISION_SCRIPT) || scripted.sources.join() !== 'model' || scripted.imageCounts.join() !== '2') {
    fail(`the vision turns did not carry the model's answers: ${JSON.stringify(scripted)}`)
  }
  if (!scripted.camera.includes('图像直接交给模型')) fail(`vision camera status: ${scripted.camera}`)
  for (const episode of visionEpisodes) {
    if (!episode.frames.every((item) => item.width === 640 && item.height === 480 && item.bytes <= IMAGE_BYTES)) {
      fail(`${episode.mode} frames: ${JSON.stringify(episode.frames)}`)
    }
  }
  if (unoffered.outcome !== '执行异常' || unoffered.agentTurns !== 1 || unoffered.steps !== 0 || !unoffered.note.includes('没有使用规则替代')) {
    fail(`an unoffered vision answer did not stop the episode: ${JSON.stringify(unoffered)}`)
  }
  const stored = JSON.parse(stores.at(-1) ?? 'null')
  if (batch.label !== '开始评测（12 局）' || batch.progress !== '已完成 12 / 12 局' || batch.stores !== 1) fail(`batch run: ${JSON.stringify(batch)}`)
  if (stored?.format !== 'g1-arm-batch-v1' || !stored.complete || stored.results.length !== 12 || JSON.stringify(stored) !== JSON.stringify(batch.exported)) {
    fail('the stored batch report is not the exported one')
  }
  const planned = ['privileged', 'rgbd'].flatMap((m) => ['transfer', 'stack', 'barrier'].flatMap((t) => [0, 1].map((s) => `${m}/${t}/${s}`)))
  if (stored.results.map((row) => `${row.observation_mode}/${row.task}/${row.seed}`).join() !== planned.join()) fail('the batch did not run the plan in order')
  const failed = stored.results.filter((row) => !row.success)
  if (failed.length) fail(`batch episodes failed: ${JSON.stringify(failed)}`)
  if (batch.rows.length !== 6 || !batch.rows.every((row) => row[1] === '2/2')) fail(`batch summary rows: ${JSON.stringify(batch.rows)}`)
  if (batch.restored.progress !== '已完成 12 / 12 局' || batch.restored.rows !== 6 || !batch.restored.exportEnabled) fail(`the report did not survive a reload: ${JSON.stringify(batch.restored)}`)
  if (historyHeading !== '第 1 步') fail(`history view heading was ${historyHeading}`)
  if (replayLabel !== '轨迹回放') fail('the timeline did not enter replay')
  if (thresholdLabel !== '0.55' || !thresholdEnabled) fail(`threshold shows ${thresholdLabel}, enabled ${thresholdEnabled}`)
  if (unsure.steps !== 0 || !unsure.thresholdEnabled || !unsure.run.includes('继续')) fail(`the unsure episode did not wait for a person: ${JSON.stringify(unsure)}`)
}

/**
 * One turn image as Studio admits it: a named JPEG data URL of at most
 * IMAGE_BYTES. The size comes from the first start-of-frame marker.
 */
function cameraFrame(image) {
  const prefix = 'data:image/jpeg;base64,'
  if (!/^[a-z][a-z0-9_-]{0,31}$/.test(image?.name ?? '') || !image.data?.startsWith(prefix)) fail(`turn image is not a named JPEG: ${image?.name}`)
  const bytes = Buffer.from(image.data.slice(prefix.length), 'base64')
  if (bytes.length > IMAGE_BYTES || bytes[0] !== 0xff || bytes[1] !== 0xd8) fail(`turn image ${image.name} is ${bytes.length} bytes or not JPEG`)
  for (let at = 2; at + 9 < bytes.length; ) {
    if (bytes[at] !== 0xff) fail(`turn image ${image.name} has a broken marker`)
    const marker = bytes[at + 1]
    if (marker >= 0xc0 && marker <= 0xc2) {
      return { name: image.name, bytes: bytes.length, height: bytes.readUInt16BE(at + 5), width: bytes.readUInt16BE(at + 7) }
    }
    at += 2 + bytes.readUInt16BE(at + 2)
  }
  fail(`turn image ${image.name} has no frame header`)
}

/**
 * The stand-in vision gateway: the request must name each frame once by
 * reference and carry no image bytes, since the box never receives them.
 */
function visionChoice(event, frames, observation, mode, step) {
  if (event.api !== 'chat' || event.body?.model !== VISION_MODEL) fail(`vision request is not a chat request: ${event.api} ${event.body?.model}`)
  if (/data:image|base64/.test(JSON.stringify(event))) fail('the vision request carried image bytes')
  const parts = event.body.messages.flatMap((message) => (Array.isArray(message.content) ? message.content : []))
  const references = parts.filter((part) => part.type === 'image_url').map((part) => part.image_url.url)
  if (JSON.stringify(references) !== JSON.stringify(frames.map(({ name }) => `aix-image:${name}`))) fail(`vision request references ${JSON.stringify(references)}`)
  if (mode === 'vision-unoffered') return 'teleport'
  const choice = VISION_SCRIPT[step] ?? 'hold'
  if (!(choice in observation.options)) fail(`the menu did not offer ${choice}`)
  return choice
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
}, 900_000).unref()

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
