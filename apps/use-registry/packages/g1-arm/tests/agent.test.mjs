import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import http from 'node:http'
import net from 'node:net'
import test from 'node:test'

import {
  DECISION_MODEL,
  DEFAULT_MODEL,
  PLANNING_SYSTEM,
  VISION_MODEL,
  choiceFromReply,
  choose,
  decisionFromReply,
  decisionRequest,
  modelApi,
  modelId,
  modelRequest,
  planFromReply,
  ruleChoice,
  turnImages,
  visionModel,
  visionRequest
} from '../agent/choose.mjs'

const HOST_DECISION_FIELDS = ['model', 'state', 'questions']

const HOST_MODEL_FIELDS = [
  'model',
  'messages',
  'stream',
  'temperature',
  'top_p',
  'max_tokens',
  'max_completion_tokens',
  'stop',
  'seed',
  'response_format',
  'reasoning_effort',
  'tools',
  'tool_choice',
  'parallel_tool_calls'
]

const observation = {
  task: '把橙色方块放到绿色目标区，然后回到初始位。',
  gripper: 'open',
  holding: false,
  options: { move_above_cube: '移到方块上方', go_home: '回到初始位' },
  baseline_choice: 'move_above_cube'
}

test('a missing observation asks for options', () => {
  assert.equal(choose('帮我看一下', undefined), '请附上一次观测。观测里要有 options。')
})

test('the rule takes the only option, then an offered baseline, then the first key', () => {
  assert.equal(ruleChoice({ options: { hold: '保持' }, baseline_choice: 'other' }), 'hold')
  assert.equal(ruleChoice({ options: { a: 'A', b: 'B' }, baseline_choice: 'b' }), 'b')
  assert.equal(ruleChoice({ options: { a: 'A', b: 'B' }, baseline_choice: 'c' }), 'a')
  assert.equal(ruleChoice({ options: {} }), null)
  assert.equal(choose('没有观测', { observation: { options: ['grasp'] } }), '{"choice":"grasp"}')
})

test('the model id is the requested safe id or the decision model', () => {
  assert.equal(DEFAULT_MODEL, DECISION_MODEL)
  assert.equal(modelId(undefined), 'platform/apofasi')
  assert.equal(modelId({ model: 'deepseek-v4-flash' }), 'deepseek-v4-flash')
  assert.equal(modelId({ model: 'bad model\n' }), DEFAULT_MODEL)
  assert.equal(modelApi(DECISION_MODEL), 'systemone')
  assert.equal(modelApi('deepseek-v4-flash'), 'chat')
})

test('the decision request asks one choice question over exactly the offered keys', () => {
  const body = decisionRequest(observation, DECISION_MODEL)
  assert.deepEqual(Object.keys(body), HOST_DECISION_FIELDS)
  assert.equal(body.model, 'platform/apofasi')
  assert.deepEqual(body.questions.action.criteria, observation.options)
  assert.equal(body.questions.action.type, 'choice')
  assert.deepEqual(body.state, { task: observation.task, gripper: 'open', holding: false })
  assert.equal(JSON.stringify(body).toLowerCase().includes('bearer'), false)
  const listed = decisionRequest({ options: ['a', 'b'], state: { phase: 'grasp' } }, DECISION_MODEL)
  assert.deepEqual(listed.questions.action.criteria, { a: null, b: null })
  assert.deepEqual(listed.state, { phase: 'grasp' })
})

test('a decision counts only with an offered choice and a distribution over exactly the offered keys', () => {
  const keys = Object.keys(observation.options)
  const reply = (choice, probabilities) => ({
    model: 'apofasi-lexical-0.1.1',
    answers: { action: { type: 'choice', choice, confidence: 0.7, probabilities } }
  })
  assert.deepEqual(decisionFromReply(200, reply('go_home', { move_above_cube: 0.3, go_home: 0.7 }), keys), {
    choice: 'go_home',
    probabilities: { move_above_cube: 0.3, go_home: 0.7 },
    confidence: 0.7,
    model: 'apofasi-lexical-0.1.1'
  })
  for (const [status, body] of [
    [401, reply('go_home', { move_above_cube: 0.3, go_home: 0.7 })],
    [200, reply('fly', { move_above_cube: 0.3, go_home: 0.7 })],
    [200, reply('go_home', { go_home: 1 })],
    [200, reply('go_home', { move_above_cube: 0.2, go_home: 0.7, fly: 0.1 })],
    [200, reply('go_home', { move_above_cube: 0.5, go_home: 0.7 })],
    [200, reply('go_home', { move_above_cube: -0.3, go_home: 1.3 })],
    [200, { answers: { action: { type: 'text', choice: 'go_home' } } }]
  ]) {
    assert.equal(decisionFromReply(status, body, keys), null, JSON.stringify(body))
  }
})

test('the model request uses only fields the host forwards and no credential', () => {
  const body = modelRequest(observation, DEFAULT_MODEL, 'rules')
  for (const key of Object.keys(body)) assert.ok(HOST_MODEL_FIELDS.includes(key), key)
  assert.equal(body.stream, false)
  assert.equal(body.messages[1].content, JSON.stringify(observation))
  assert.equal(JSON.stringify(body).toLowerCase().includes('bearer'), false)
})

test('a model reply counts only when it names one offered key', () => {
  const keys = Object.keys(observation.options)
  const reply = (content) => ({ choices: [{ message: { content } }] })
  assert.equal(choiceFromReply(200, reply('{"choice":"go_home"}'), keys), 'go_home')
  assert.equal(choiceFromReply(200, reply('```json\n{"choice":"go_home"}\n```'), keys), 'go_home')
  assert.equal(choiceFromReply(200, reply('{"choice":"fly"}'), keys), null)
  assert.equal(choiceFromReply(200, reply('go_home'), keys), null)
  assert.equal(choiceFromReply(200, reply(''), keys), null)
  assert.equal(choiceFromReply(401, reply('{"choice":"go_home"}'), keys), null)
})

const planning = {
  state: { observation: { tcp: [0.38, -0.08, 0.22], gripper: 'open' }, recent_outcomes: [] },
  decision: { type: 'choice', instructions: 'Plan toward the goal.', criteria: { z_neg_40: '{}', close: '{}' } },
  options: { z_neg_40: '{}', close: '{}' },
  baseline_choice: null,
  control: 'incremental'
}

// Catch image bytes in the box's request, an unlabelled view, or a decision model asked to see.
test('a vision request labels each view and names images only by host reference', () => {
  assert.deepEqual(turnImages(undefined), [])
  assert.deepEqual(turnImages([{ name: 'external' }, { name: 'wrist' }]), ['external', 'wrist'])
  for (const manifest of [[{ name: 'a' }, { name: 'a' }], [{ name: 'A' }], [{}], {}, [{ name: 'a' }, { name: 'b' }, { name: 'c' }]]) {
    assert.equal(turnImages(manifest), null, JSON.stringify(manifest))
  }
  assert.equal(visionModel(undefined), VISION_MODEL)
  assert.equal(visionModel({ model: DECISION_MODEL }), VISION_MODEL)
  assert.equal(visionModel({ model: 'qwen-vl-max' }), 'qwen-vl-max')

  const body = visionRequest(planning, VISION_MODEL, ['external', 'wrist'])
  for (const key of Object.keys(body)) assert.ok(HOST_MODEL_FIELDS.includes(key), key)
  assert.equal(body.stream, false)
  assert.ok(body.max_tokens >= 2000)
  assert.equal(body.reasoning_effort, 'none', 'hidden reasoning used all 4096 tokens on the gateway and gave no answer')
  assert.ok(body.messages[0].content.startsWith(PLANNING_SYSTEM))
  const [text, ...views] = body.messages[1].content
  assert.deepEqual(JSON.parse(text.text), { state: planning.state, decision: planning.decision })
  assert.deepEqual(views, [
    { type: 'text', text: 'Camera view: external' },
    { type: 'image_url', image_url: { url: 'aix-image:external' } },
    { type: 'text', text: 'Camera view: wrist' },
    { type: 'image_url', image_url: { url: 'aix-image:wrist' } }
  ])
  assert.equal(JSON.stringify(body).includes('base64'), false)
})

// Catch a truncated, padded, or invented planning answer being executed.
test('a planning answer counts only as one finished offered choice with short public summaries', () => {
  const keys = Object.keys(planning.options)
  const reply = (content, finish_reason = 'stop') => ({ choices: [{ finish_reason, message: { content } }] })
  assert.deepEqual(planFromReply(200, reply('{"choice":"close","intent":"  夹住  方块 ","visual_evidence":"红块在两指之间"}'), keys), {
    choice: 'close',
    intent: '夹住 方块',
    visual_evidence: '红块在两指之间'
  })
  assert.deepEqual(planFromReply(200, reply('```json\n{"choice":"z_neg_40"}\n```'), keys), { choice: 'z_neg_40' })
  for (const [status, body] of [
    [200, reply('{"choice":"close"}', 'length')],
    [200, reply('{"choice":"fly"}')],
    [200, reply('{"choice":"close","probabilities":{"close":1}}')],
    [200, reply(`{"choice":"close","intent":"${'x'.repeat(241)}"}`)],
    [200, reply('{"choice":"close","intent":"data:image/png;base64,AAAA"}')],
    [200, { choices: [] }],
    [200, { choices: [reply('{"choice":"close"}').choices[0], reply('{"choice":"close"}').choices[0]] }],
    [401, reply('{"choice":"close"}')]
  ]) {
    assert.equal(planFromReply(status, body, keys), null, JSON.stringify(body))
  }
})

// Catch a camera turn falling back to the rule, or reaching the decision model.
test('a camera turn asks the vision model and never substitutes a rule choice', async (t) => {
  const agent = await start()
  t.after(() => agent.child.kill())
  const images = [
    { name: 'external', media_type: 'image/jpeg', bytes: 4096 },
    { name: 'wrist', media_type: 'image/jpeg', bytes: 2048 }
  ]
  await agent.post('/v1/agent/commands', { ...command('see-1', planning), images })
  const request = await waitFor(agent, 0, (event) => event.type === 'model_request')
  assert.equal(request.api, 'chat')
  assert.equal(request.body.model, VISION_MODEL)
  assert.deepEqual(
    request.body.messages[1].content.filter((part) => part.type === 'image_url').map((part) => part.image_url.url),
    ['aix-image:external', 'aix-image:wrist']
  )
  await agent.post('/v1/agent/model-responses', {
    request: request.request,
    status: 200,
    body: { choices: [{ finish_reason: 'stop', message: { content: '{"choice":"z_neg_40","intent":"下降"}' } }] }
  })
  const done = await waitFor(agent, request.sequence, (event) => event.type === 'completed')
  const output = (await agent.page(request.sequence)).find((event) => event.type === 'output')
  assert.deepEqual(JSON.parse(output.text), { choice: 'z_neg_40', intent: '下降', source: 'model', model: VISION_MODEL, images: 2 })

  await agent.post('/v1/agent/commands', { ...command('see-2', planning), images })
  const second = await waitFor(agent, done.sequence, (event) => event.type === 'model_request')
  await agent.post('/v1/agent/model-responses', { request: second.request, status: 502, body: { error: 'bad_gateway' } })
  const ended = await waitFor(agent, second.sequence, (event) => event.type === 'completed')
  const refused = (await agent.page(second.sequence)).find((event) => event.type === 'output' && event.sequence < ended.sequence)
  assert.deepEqual(JSON.parse(refused.text), { error: 'model unavailable (502)', model: VISION_MODEL })

  await agent.post('/v1/agent/commands', { ...command('see-3', planning), images: [{ name: '../x' }] })
  const failed = await waitFor(agent, ended.sequence, (event) => event.type === 'failed')
  assert.equal(failed.message, 'the image manifest is malformed')
})

test('the warm entry serves many runs with one process-wide sequence', async (t) => {
  const agent = await start()
  t.after(() => agent.child.kill())

  const first = await agent.post('/v1/agent/commands', command('run-1', observation))
  assert.equal(first.status, 200)
  const request = await waitFor(agent, 0, (event) => event.type === 'model_request')
  assert.equal(request.run_id, 'run-1')
  assert.equal(request.sequence, 1)
  assert.equal(request.api, 'systemone')
  assert.deepEqual(Object.keys(request.body), HOST_DECISION_FIELDS)
  assert.equal(request.body.model, DECISION_MODEL)
  const answered = await agent.post('/v1/agent/model-responses', {
    request: request.request,
    status: 200,
    body: {
      model: 'apofasi-lexical-0.1.1',
      answers: {
        action: { type: 'choice', choice: 'go_home', confidence: 0.7, probabilities: { move_above_cube: 0.3, go_home: 0.7 } }
      }
    }
  })
  assert.equal(answered.status, 200)
  const done = await waitFor(agent, 1, (event) => event.type === 'completed')
  const output = (await agent.page(1)).find((event) => event.type === 'output')
  assert.deepEqual(JSON.parse(output.text), {
    choice: 'go_home',
    probabilities: { move_above_cube: 0.3, go_home: 0.7 },
    confidence: 0.7,
    model: 'apofasi-lexical-0.1.1',
    source: 'model'
  })
  assert.equal(output.run_id, 'run-1')

  await agent.post('/v1/agent/commands', command('run-2', observation, { model: 'deepseek-v4-flash' }))
  const second = await waitFor(agent, done.sequence, (event) => event.type === 'model_request')
  assert.equal(second.run_id, 'run-2')
  assert.ok(second.sequence > done.sequence)
  assert.equal(second.api, 'chat')
  assert.equal(second.body.model, 'deepseek-v4-flash')
  for (const key of Object.keys(second.body)) assert.ok(HOST_MODEL_FIELDS.includes(key), key)
  assert.ok(second.body.messages[0].content.includes('{"choice"'))
  await agent.post('/v1/agent/model-responses', { request: second.request, status: 401, body: { error: 'unauthorized' } })
  await waitFor(agent, second.sequence, (event) => event.type === 'completed')
  const fallback = (await agent.page(second.sequence)).find((event) => event.type === 'output')
  assert.deepEqual(JSON.parse(fallback.text), {
    choice: 'move_above_cube',
    source: 'rule',
    note: 'model unavailable (401)'
  })

  const sequenceBefore = (await agent.page(0)).at(-1).sequence
  await agent.post('/v1/agent/commands', command('run-3', { options: { open_gripper: '张开夹爪' } }))
  const single = await waitFor(agent, sequenceBefore, (event) => event.type === 'completed')
  const singleEvents = (await agent.page(sequenceBefore)).filter((event) => event.sequence <= single.sequence)
  assert.equal(singleEvents.some((event) => event.type === 'model_request'), false)
  assert.equal(JSON.parse(singleEvents.find((event) => event.type === 'output').text).choice, 'open_gripper')
})

test('the entry refuses malformed, duplicate, and unknown traffic', async (t) => {
  const agent = await start()
  t.after(() => agent.child.kill())
  assert.equal((await agent.post('/v1/agent/commands', { prompt: 'x' })).status, 400)
  assert.equal((await agent.post('/v1/agent/commands', command('dup', { options: { a: 'A' } }))).status, 200)
  assert.equal((await agent.post('/v1/agent/commands', command('dup', { options: { a: 'A' } }))).status, 409)
  assert.equal((await agent.post('/v1/agent/model-responses', { request: 'nope', status: 200 })).status, 404)
  assert.equal((await agent.post('/v1/elsewhere', {})).status, 404)
  await agent.post('/v1/agent/commands', { prompt: 'no observation', identity: { run_id: 'empty' } })
  const failed = await waitFor(agent, 0, (event) => event.type === 'failed')
  assert.equal(failed.run_id, 'empty')
})

// Studio's edge client writes one HTTP/1.1 request with Connection: close and
// parses everything after the header block as one JSON value.
test('every reply is one Content-Length JSON body the host reads as written', async (t) => {
  const agent = await start()
  t.after(() => agent.child.kill())
  const exchanges = [
    ['/v1/agent/commands', command('raw', { options: { a: 'A' } })],
    ['/v1/agent/events:page', { after: 0 }],
    ['/v1/agent/model-responses', { request: 'nope', status: 200 }]
  ]
  for (const [path, body] of exchanges) {
    const raw = await rawPost(agent.port, path, body)
    const split = raw.indexOf('\r\n\r\n')
    const head = raw.slice(0, split).toLowerCase()
    assert.doesNotMatch(head, /transfer-encoding/, path)
    const length = Number(/content-length: (\d+)/.exec(head)?.[1])
    const text = raw.slice(split + 4)
    assert.equal(Buffer.byteLength(text), length, path)
    assert.doesNotThrow(() => JSON.parse(text), path)
  }
})

function rawPost(port, path, body) {
  const payload = JSON.stringify(body)
  return new Promise((resolve, reject) => {
    const socket = net.connect(port, '127.0.0.1', () => {
      socket.end(
        `POST ${path} HTTP/1.1\r\nHost: 127.0.0.1:${port}\r\nContent-Type: application/json\r\nContent-Length: ${Buffer.byteLength(payload)}\r\nConnection: close\r\n\r\n${payload}`
      )
    })
    const chunks = []
    socket.on('data', (chunk) => chunks.push(chunk))
    socket.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')))
    socket.on('error', reject)
  })
}

function command(runId, value, extra = {}) {
  return {
    schema: 'aster.code.agent.start.v1',
    request_id: runId,
    identity: { session_id: 'session', run_id: runId },
    prompt: '选择下一步动作。',
    parameters: { observation: value, ...extra }
  }
}

async function start() {
  const child = spawn(process.execPath, [new URL('../agent/run.mjs', import.meta.url).pathname], {
    stdio: ['ignore', 'pipe', 'inherit']
  })
  const port = await new Promise((resolve, reject) => {
    let text = ''
    child.stdout.on('data', (chunk) => {
      text += chunk.toString('utf8')
      const match = /port (\d+)/.exec(text)
      if (match) resolve(Number(match[1]))
    })
    child.once('exit', () => reject(new Error('agent exited')))
  })
  const post = (path, body) => send(port, path, body)
  return {
    child,
    port,
    post,
    page: async (after) => (await post('/v1/agent/events:page', { after })).body.events
  }
}

async function waitFor(agent, after, matches) {
  const deadline = Date.now() + 5000
  while (Date.now() < deadline) {
    const found = (await agent.page(after)).find(matches)
    if (found) return found
    await new Promise((resolve) => setTimeout(resolve, 10))
  }
  throw new Error('event did not arrive')
}

function send(port, path, body) {
  const payload = JSON.stringify(body)
  return new Promise((resolve, reject) => {
    const request = http.request(
      {
        host: '127.0.0.1',
        port,
        path,
        method: 'POST',
        headers: { 'content-type': 'application/json', 'content-length': Buffer.byteLength(payload) }
      },
      (response) => {
        const chunks = []
        response.on('data', (chunk) => chunks.push(chunk))
        response.on('end', () => {
          try {
            resolve({ status: response.statusCode, body: JSON.parse(Buffer.concat(chunks).toString('utf8')) })
          } catch (error) {
            reject(error)
          }
        })
      }
    )
    request.on('error', reject)
    request.end(payload)
  })
}
