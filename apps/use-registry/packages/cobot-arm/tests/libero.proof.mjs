// LIBERO browser proof: the libero_90 scenes render and run in Chromium under
// Studio's view policy, with the real agent/run.mjs behind the AIX turn loop.
//
//   node tests/libero.proof.mjs
//
// 1. Each scene's first planner turn carries the external and wrist images;
//    they are compared with robosuite's own renders of the same init state.
// 2. A vision episode on the drawer: the stand-in gateway sees only the chat
//    request (state text plus image references) and answers world waypoints,
//    so the whole chain runs: capture, agent, plan, grounding, OSC, predicate.
// 3. The gateway answers 401: the episode stops with no motion and no rule.
// 4. A batch of the reference planner on both tasks: its waypoints are
//    grounded through rendered depth when a camera sees them.
import { readFileSync, writeFileSync } from 'node:fs'
import { randomBytes } from 'node:crypto'
import { spawn } from 'node:child_process'
import { join } from 'node:path'
import { playwright, root, serveView } from './view-host.mjs'

const { chromium } = playwright
const WIDTH = 1440
const HEIGHT = 900
const VISION_MODEL = 'bailian/deepseek-v4.1-flash'
const IMAGE_BYTES = 256 * 1024
// 16 px block and whole-image colour error against robosuite's own render. The
// planner images carry the pixel grid, the TCP cross and JPEG loss, so they
// never match exactly.
const FIDELITY = { block_mae: 15, mean_colour_error: 10 }
const token = randomBytes(32).toString('hex')
const documentId = randomBytes(32).toString('hex')
const shots = process.env.PROOF_SCREENSHOTS ?? '/tmp/cobot-arm-libero-proof'

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

// The drawer waypoints as world targets: close, above the drawer front, down,
// push along -y, back up. The gateway advances once the TCP is within 5 mm.
const DRAWER_SCRIPT = [null, [0, 0, 1.178], [0, 0, 1.115], [0, -0.25, 1.115], [0, -0.1, 1.178]]

function scriptedPlan(state, script) {
  const previous = state.previous_plan
  let index = previous ? Number(/^proof (\d+)$/.exec(previous.stage)?.[1] ?? 0) : 0
  const close = previous && Math.max(...previous.target_xyz.map((value, i) => Math.abs(value - state.robot.tcp[i]))) <= 0.005
  const stalled = state.recent_motion.length === 3 && state.recent_motion.every((motion) => Math.hypot(...motion.delta_m) < 0.001)
  if (previous && (close || stalled || index === 0)) index = Math.min(index + 1, script.length - 1)
  const xyz = script[index]
  return {
    stage: `proof ${index}`,
    intent: xyz ? 'Move to the next scripted waypoint.' : 'Close the gripper in place.',
    visual_evidence: 'Stand-in gateway; the images were referenced, not read.',
    target: xyz ? { kind: 'world', xyz_m: xyz } : { kind: 'relative', delta_m: [0, 0, 0] },
    rotation_delta: [0, 0, 0],
    gripper: 'close',
    max_motor_steps: xyz ? 8 : 2
  }
}

function cameraFrame(image) {
  const prefix = 'data:image/jpeg;base64,'
  if (!/^[a-z][a-z0-9_-]{0,31}$/.test(image?.name ?? '') || !image.data?.startsWith(prefix)) fail(`turn image is not a named JPEG: ${image?.name}`)
  const bytes = Buffer.from(image.data.slice(prefix.length), 'base64')
  if (bytes.length > IMAGE_BYTES || bytes[0] !== 0xff || bytes[1] !== 0xd8) fail(`turn image ${image.name} is ${bytes.length} bytes or not JPEG`)
  for (let at = 2; at + 9 < bytes.length; ) {
    if (bytes[at] !== 0xff) fail(`turn image ${image.name} has a broken marker`)
    const marker = bytes[at + 1]
    if (marker >= 0xc0 && marker <= 0xc2) {
      return { name: image.name, bytes, height: bytes.readUInt16BE(at + 5), width: bytes.readUInt16BE(at + 7) }
    }
    at += 2 + bytes.readUInt16BE(at + 2)
  }
  fail(`turn image ${image.name} has no frame header`)
}

/** Mean absolute difference of 16 px block means, 0-255, and of the whole-image mean colour. */
async function compareImages(page, ours, theirs) {
  return page.evaluate(
    async ([a, b]) => {
      const pixels = async (url) => {
        const image = new Image()
        image.src = url
        await image.decode()
        const canvas = new OffscreenCanvas(image.naturalWidth, image.naturalHeight)
        const context = canvas.getContext('2d')
        context.drawImage(image, 0, 0)
        return { width: image.naturalWidth, data: context.getImageData(0, 0, image.naturalWidth, image.naturalHeight).data }
      }
      const [x, y] = await Promise.all([pixels(a), pixels(b)])
      if (x.width !== y.width) return { error: `sizes ${x.width} vs ${y.width}` }
      const size = x.width
      const block = 16
      const blocks = size / block
      const means = (p) => {
        const out = new Float64Array(blocks * blocks * 3)
        for (let v = 0; v < size; v++)
          for (let u = 0; u < size; u++)
            for (let c = 0; c < 3; c++) out[(Math.floor(v / block) * blocks + Math.floor(u / block)) * 3 + c] += p.data[(v * size + u) * 4 + c] / (block * block)
        return out
      }
      const [mx, my] = [means(x), means(y)]
      let blockError = 0
      for (let i = 0; i < mx.length; i++) blockError += Math.abs(mx[i] - my[i])
      const average = (m) => [0, 1, 2].map((c) => m.filter((_, i) => i % 3 === c).reduce((s, v) => s + v, 0) / (blocks * blocks))
      const [ax, ay] = [average(mx), average(my)]
      return {
        block_mae: Number((blockError / mx.length).toFixed(1)),
        mean_rgb: ax.map((v) => Math.round(v)),
        robosuite_mean_rgb: ay.map((v) => Math.round(v)),
        mean_colour_error: Number(Math.max(...ax.map((v, i) => Math.abs(v - ay[i]))).toFixed(1))
      }
    },
    [ours, theirs]
  )
}

async function main() {
  const agent = await startAgent()
  const { server, origin } = await serveView({ token, documentId, width: WIDTH, height: HEIGHT })
  const browser = await chromium.launch({ args: ['--enable-unsafe-swiftshader', '--use-angle=swiftshader'] })
  const page = await browser.newPage({ viewport: { width: WIDTH, height: HEIGHT } })
  const problems = []
  page.on('console', (message) => {
    if (message.type() === 'error' || /Refused|Content Security Policy/.test(message.text())) problems.push(message.text())
  })
  page.on('pageerror', (error) => problems.push(String(error)))

  let gateway = 'script'
  let after = 0
  let runs = 0
  const turns = []
  const firstImages = new Map()
  let viewState = null
  await page.exposeFunction('viewState', (...value) => {
    if (value.length === 0) return viewState
    viewState = value[0]
    return true
  })
  await page.exposeFunction('turn', async ({ agent: name, prompt, parameters, images }) => {
    if (name !== 'cobot-arm') return { reason: 'not_found' }
    const observation = parameters?.observation
    if (observation?.protocol !== 'libero-rgbd-waypoint-v1' || !observation.state?.robot) fail('turn is not a LIBERO plan')
    const frames = (images ?? []).map(cameraFrame)
    if (frames.map((frame) => frame.name).join() !== 'external,wrist') fail(`turn images ${frames.map((frame) => frame.name)}`)
    if (!frames.every((frame) => frame.width === 384 && frame.height === 384)) fail('turn images are not 384 px')
    const task = observation.state.task
    if (observation.state.robot.step === 0 && !firstImages.has(task)) firstImages.set(task, images)
    const runId = `libero-run-${++runs}`
    const command = await agent.post('/v1/agent/commands', {
      schema: 'aster.code.agent.v1',
      request_id: `libero-request-${runs}`,
      identity: { session_id: 'libero-proof', run_id: runId },
      prompt,
      parameters,
      images: frames.map(({ name, bytes }) => ({ name, media_type: 'image/jpeg', bytes: bytes.length }))
    })
    if (command.status !== 200) return { reason: 'agent_failed' }
    for (let poll = 0; poll < 2000; poll++) {
      const events = (await agent.post('/v1/agent/events:page', { after })).body.events
      for (const event of events) {
        after = event.sequence
        if (event.run_id !== runId) continue
        if (event.type === 'model_request') {
          if (event.api !== 'chat' || event.body?.model !== VISION_MODEL) fail(`plan request is not a vision chat: ${event.api} ${event.body?.model}`)
          if (/data:image|base64/.test(JSON.stringify(event))) fail('the plan request carried image bytes')
          const parts = event.body.messages.flatMap((message) => (Array.isArray(message.content) ? message.content : []))
          const references = parts.filter((part) => part.type === 'image_url').map((part) => part.image_url.url)
          if (references.join() !== 'aix-image:external,aix-image:wrist') fail(`plan request references ${references}`)
          const state = JSON.parse(parts[0].text)
          const answer =
            gateway === 'script'
              ? { status: 200, body: { choices: [{ finish_reason: 'stop', message: { role: 'assistant', content: JSON.stringify(scriptedPlan(state, DRAWER_SCRIPT)) } }] } }
              : { status: 401, body: { error: 'missing bearer' } }
          const answered = await agent.post('/v1/agent/model-responses', { request: event.request, ...answer })
          if (answered.status !== 200) fail(`model response refused: ${answered.status}`)
        } else if (event.type === 'output') {
          turns.push({ gateway, run: runId, step: observation.state.robot.step, ...JSON.parse(event.text) })
          return { text: event.text }
        } else if (event.type === 'failed') {
          turns.push({ gateway, run: runId, failed: event.message })
          return { reason: 'agent_failed' }
        }
      }
      await new Promise((resolve) => setTimeout(resolve, 5))
    }
    return { reason: 'agent_failed' }
  })

  await page.goto(`${origin}/host.html`)
  const frame = page.frameLocator('#view')
  const statusText = frame.locator('#status-text')
  await frame.locator('#loading.hidden').waitFor({ state: 'attached', timeout: 30_000 })
  await frame.locator('#run:not([disabled])').waitFor({ timeout: 10_000 })
  progress('view ready')

  const scenes = {}
  const finish = async (label) => {
    try {
      await statusText.filter({ hasText: /验证通过|执行异常|预算耗尽/ }).waitFor({ timeout: 300_000 })
    } catch {
      fail(`${label} stuck: ${await statusText.textContent()} note=${await frame.locator('#decision-note').textContent()} problems=${JSON.stringify(problems)}`)
    }
    return statusText.textContent()
  }

  for (const [task, name, language] of [
    ['libero-microwave', 'microwave', 'close the microwave'],
    ['libero-drawer', 'drawer', 'close the top drawer of the cabinet']
  ]) {
    const started = performance.now()
    await frame.locator(`[data-task="${task}"]`).click()
    await frame.locator('#libero-settings:not([hidden])').waitFor({ timeout: 10_000 })
    await frame.locator('#loading.hidden').waitFor({ state: 'attached', timeout: 60_000 })
    await statusText.filter({ hasText: '待命' }).waitFor({ timeout: 10_000 })
    const loadMs = Math.round(performance.now() - started)
    await page.waitForTimeout(300)
    await page.screenshot({ path: `${shots}-${name}-ready.png` })
    // One vision turn at step 0 delivers both cameras of the init state; the
    // stand-in then refuses, so this run only photographs the scene.
    gateway = 'refuse'
    await frame.locator('#run').click()
    const outcome = await finish(`${name} photo`)
    const images = firstImages.get(language)
    if (!images) fail(`${name}: no step-0 images arrived (tasks seen: ${[...firstImages.keys()]})`)
    const fidelity = {}
    for (const image of images) {
      writeFileSync(`${shots}-${name}-${image.name}.jpg`, Buffer.from(image.data.split(',')[1], 'base64'))
      const reference = readFileSync(join(root, `tests/fixtures/libero/${name}-robosuite-${image.name}.png`))
      fidelity[image.name] = await compareImages(page, image.data, `data:image/png;base64,${reference.toString('base64')}`)
    }
    scenes[name] = {
      loadMs,
      refusal: outcome,
      steps: Number(/^(\d+) \//.exec(await frame.locator('#run-budget').textContent())?.[1] ?? NaN),
      note: await frame.locator('#decision-note').textContent(),
      fidelity
    }
    progress(`${name}: loaded in ${loadMs} ms, ${JSON.stringify(fidelity)}`)
  }

  // Vision on the drawer through the real agent with a scripted gateway.
  gateway = 'script'
  const before = turns.length
  await frame.locator('#reset').click()
  await statusText.filter({ hasText: '待命' }).waitFor({ timeout: 10_000 })
  const visionStarted = performance.now()
  await frame.locator('#run').click()
  const visionOutcome = await finish('vision drawer')
  const visionTurns = turns.slice(before)
  const vision = {
    outcome: visionOutcome,
    turns: visionTurns.length,
    sources: [...new Set(visionTurns.map((turn) => turn.source ?? turn.error ?? turn.failed))],
    plans: await frame.locator('#events li').count(),
    metric: await frame.locator('#lift').textContent(),
    predicate: await frame.locator('#support').textContent(),
    wallMs: Math.round(performance.now() - visionStarted)
  }
  await page.screenshot({ path: `${shots}-vision-drawer.png` })
  await frame.locator('#events li button').first().click()
  vision.historyHeading = await frame.locator('#decision-heading').textContent()
  vision.historyCamera = await frame.locator('#camera-status').textContent()
  await frame.locator('#decision-live').click()
  progress(`vision drawer: ${JSON.stringify(vision)}`)

  // The reference planner on both tasks as a batch, with rendered depth.
  await frame.locator('[data-planner="reference"]').click()
  await statusText.filter({ hasText: '待命' }).waitFor({ timeout: 10_000 })
  const planner = await frame.locator('#planner-help').textContent()
  await frame.locator('#eval-block summary').click()
  await frame.locator('[data-eval-planner="vision"]').click()
  await frame.locator('#eval-runs').evaluate((input) => {
    input.value = '2'
    input.dispatchEvent(new Event('input'))
  })
  const batchLabel = await frame.locator('#eval-run-label').textContent()
  const batchStarted = performance.now()
  await frame.locator('#eval-run').click()
  try {
    await frame.locator('#eval-progress').filter({ hasText: /^已(完成|停止)/ }).waitFor({ timeout: 600_000 })
  } catch {
    fail(`batch stuck: ${await frame.locator('#eval-progress').textContent()} problems=${JSON.stringify(problems)}`)
  }
  await page.screenshot({ path: `${shots}-batch.png` })
  await frame.locator('#eval-export-open').click()
  const report = JSON.parse(await frame.locator('#eval-json').inputValue())
  await frame.locator('#eval-export-close').click()
  const batch = {
    label: batchLabel,
    planner,
    progress: await frame.locator('#eval-progress').textContent(),
    wallMs: Math.round(performance.now() - batchStarted),
    rows: report.results.map(({ task, init, planner: p, success, steps, plans, pixel_targets, goal_qpos }) => ({ task, init, planner: p, success, steps, plans, pixel_targets, goal_qpos })),
    stored: viewState !== null && JSON.parse(viewState).format === report.format
  }
  progress(`batch: ${JSON.stringify(batch)}`)

  // A single episode from another init state: the slider resets the episode there.
  await frame.locator('#libero-init').evaluate((input) => {
    input.value = '9'
    input.dispatchEvent(new Event('input'))
    input.dispatchEvent(new Event('change'))
  })
  await statusText.filter({ hasText: '待命' }).waitFor({ timeout: 10_000 })
  await frame.locator('#libero-init-value').filter({ hasText: /^9$/ }).waitFor({ timeout: 10_000 })
  const initNine = { slider: await frame.locator('#libero-init').inputValue(), metric: await frame.locator('#lift').textContent() }
  progress(`init 9: ${JSON.stringify(initNine)}`)

  await browser.close()
  server.close()
  agent.child.kill()

  const result = { scenes, vision, batch, initNine, turns: turns.length, problems }
  console.log(JSON.stringify(result, null, 2))
  if (problems.length) fail('console reported errors or policy violations')
  for (const [name, scene] of Object.entries(scenes)) {
    if (scene.refusal !== '执行异常' || scene.steps !== 0 || !scene.note.includes('没有使用规则替代')) fail(`${name}: the refused turn moved or fell back`)
    for (const [view, score] of Object.entries(scene.fidelity)) {
      if (score.error || score.block_mae > FIDELITY.block_mae || score.mean_colour_error > FIDELITY.mean_colour_error) fail(`${name} ${view} render drifts from robosuite: ${JSON.stringify(score)}`)
    }
  }
  if (vision.outcome !== '验证通过' || vision.predicate !== '满足') fail('the vision drawer episode did not close the drawer')
  if (vision.sources.length !== 1 || vision.sources[0] !== 'model') fail(`vision turns came from ${vision.sources}`)
  if (!vision.historyHeading.startsWith('第 1 次规划') || !vision.historyCamera.includes('第 0 步')) fail('the first plan does not show its own camera input')
  if (batch.rows.length !== 4 || !batch.rows.every((row) => row.planner === 'reference' && row.success)) fail('the reference batch did not close both tasks')
  if (batch.rows.map((row) => row.init).join() !== '0,1,0,1') fail(`batch runs did not start from init states 0 and 1: ${batch.rows.map((row) => row.init)}`)
  if (!batch.rows.some((row) => row.pixel_targets > 0)) fail('no reference waypoint was grounded through rendered depth')
  if (!batch.stored) fail('the batch report was not stored as view state')
  if (initNine.slider !== '9') fail(`the init slider did not keep init state 9: ${initNine.slider}`)
  // The microwave's init state 9 opens the door to -1.737 rad.
  if (initNine.metric !== '-100') fail(`init state 9 did not reset the door: ${initNine.metric}`)
}

setTimeout(() => {
  console.error('proof: timed out')
  process.exit(1)
}, 900_000).unref()

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
