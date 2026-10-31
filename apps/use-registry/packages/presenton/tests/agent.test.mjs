import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import http from 'node:http'
import test from 'node:test'

import {
  DEFAULT_MODEL,
  LANGUAGES,
  MAX_DECK_SLIDES,
  MAX_REQUESTS_PER_TURN,
  feedbackMessage,
  modelId,
  modelRequest,
  normalizeStructure,
  outlineSchema,
  replyObject,
  runStep,
  settingsFrom,
  slideSchema,
  slideSystemPrompt,
  validate
} from '../agent/deck.mjs'
import { ICON_NAMES } from '../agent/icons.mjs'
import { TEMPLATES } from '../agent/templates.mjs'
import { fakeReply, sample } from './fake-model.mjs'

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
const HOST_BODY_MAX = 256 * 1024
const CHINESE = 'Chinese (Simplified - 中文, 汉语)'
const ENGLISH = 'English'

const outlineParameters = {
  step: 'outline',
  content: '给高中生讲量子计算入门',
  n_slides: 5,
  language: CHINESE,
  tone: 'educational',
  verbosity: 'standard',
  include_title_slide: true
}

const chat = (content, finish_reason = 'stop') => ({
  status: 200,
  body: { choices: [{ finish_reason, message: { role: 'assistant', content: typeof content === 'string' ? content : JSON.stringify(content) } }] }
})

function recorder(answer) {
  const bodies = []
  const ask = async (body) => {
    bodies.push(body)
    return answer(body, bodies.length)
  }
  return { ask, bodies }
}

test('settings are a closed set and a bad field is named', () => {
  assert.equal(settingsFrom(outlineParameters).n_slides, 5)
  assert.equal(settingsFrom({ ...outlineParameters, n_slides: null }).n_slides, null)
  assert.equal(LANGUAGES.length, 90)
  for (const language of [CHINESE, ENGLISH, 'Auto (English)', 'Samoan (Gagana Samoa)']) {
    assert.equal(settingsFrom({ ...outlineParameters, language }).language, language)
  }
  for (const [patch, message] of [
    [{ step: 'export' }, /step must be one of: outline, structure, slide/],
    [{ n_slides: 0 }, /n_slides must be null or an integer from 1 to 20/],
    [{ n_slides: MAX_DECK_SLIDES + 1 }, /n_slides/],
    [{ language: 'Klingon' }, /language must be one of/],
    [{ tone: 'angry' }, /tone must be one of/],
    [{ verbosity: 'verbose' }, /verbosity must be one of/],
    [{ content: '' }, /content is required/]
  ]) {
    assert.throws(() => settingsFrom({ ...outlineParameters, ...patch }), message, JSON.stringify(patch))
  }
  assert.throws(() => settingsFrom({ step: 'structure', template: 'neon', outline: ['a'] }), /template must be one of: general, modern, standard, swift/)
  assert.throws(() => settingsFrom({ step: 'slide', template: 'general', layout: 'nope', outline: 'a', slide_number: 1, language: ENGLISH }), /layout must be a layout id/)
})

test('the model id is the requested safe id or the default', () => {
  assert.equal(modelId(undefined), DEFAULT_MODEL)
  assert.equal(modelId({ model: 'deepseek-v4-flash' }), 'deepseek-v4-flash')
  assert.equal(modelId({ model: 'bad model\n' }), DEFAULT_MODEL)
})

test('a model request uses only fields the host forwards, never streams, and carries no credential', () => {
  const body = modelRequest(DEFAULT_MODEL, [{ role: 'user', content: 'hi' }], 4096)
  for (const key of Object.keys(body)) assert.ok(HOST_MODEL_FIELDS.includes(key), key)
  assert.equal(body.stream, false)
  assert.equal(body.reasoning_effort, 'none')
  assert.deepEqual(body.response_format, { type: 'json_object' })
  assert.equal(JSON.stringify(body).toLowerCase().includes('bearer'), false)
})

test('every slide request of every layout fits the host body limit', () => {
  for (const [id, template] of Object.entries(TEMPLATES)) {
    for (const layout of template.layouts) {
      const settings = settingsFrom({ step: 'slide', template: id, layout: layout.id, outline: '## 标题\n- 内容'.repeat(200), slide_number: 3, language: CHINESE })
      const schema = slideSchema(settings.layout, settings.language)
      const size = Buffer.byteLength(JSON.stringify(modelRequest(DEFAULT_MODEL, [{ role: 'system', content: slideSystemPrompt(settings, schema) }], 4096)))
      assert.ok(size < HOST_BODY_MAX, `${id}/${layout.id} is ${size} bytes`)
    }
  }
})

test('Chinese, Japanese, and Korean halve the text limits; other languages keep them', () => {
  const english = outlineSchema(4, ENGLISH).properties.slides.items.properties.content
  const chinese = outlineSchema(4, CHINESE).properties.slides.items.properties.content
  assert.deepEqual([english.minLength, english.maxLength], [100, 1200])
  assert.deepEqual([chinese.minLength, chinese.maxLength], [50, 600])
  assert.equal(outlineSchema(4, 'Korean (한국어)').properties.slides.items.properties.content.maxLength, 600)
  assert.equal(outlineSchema(4, 'German (Deutsch)').properties.slides.items.properties.content.maxLength, 1200)
})

test('icon fields accept only vendored icon names, and the prompt lists them', () => {
  const [templateId, layout] = Object.entries(TEMPLATES)
    .flatMap(([id, template]) => template.layouts.map((entry) => [id, entry]))
    .find(([, entry]) => JSON.stringify(entry.schema).includes('icon_query'))
  const settings = settingsFrom({ step: 'slide', template: templateId, layout: layout.id, outline: '## A', slide_number: 2, language: ENGLISH })
  const schema = slideSchema(settings.layout, ENGLISH)
  const prompt = slideSystemPrompt(settings, schema)
  assert.ok(prompt.includes('# Icon Names:'))
  assert.ok(prompt.includes(ICON_NAMES[0]) && prompt.includes(ICON_NAMES.at(-1)))
  assert.ok(prompt.includes('Exactly one name from the Icon Names list'))
  assert.doesNotMatch(prompt, /"icon":true/)
  const good = sample(JSON.parse(JSON.stringify(schema).replaceAll('{"type":"string","icon":true}', '{"type":"string","description":"Exactly one name from the Icon Names list"}')))
  assert.deepEqual(validate(schema, good), [])
  const bad = JSON.parse(JSON.stringify(good).replace(/"icon_query":"[^"]+"/, '"icon_query":"rocket-ship-3000"'))
  assert.match(validate(schema, bad).join('\n'), /"rocket-ship-3000" is not one of the Icon Names/)
})

test('validation names the path and the broken rule', () => {
  const schema = {
    type: 'object',
    required: ['title', 'items'],
    properties: {
      title: { type: 'string', minLength: 3, maxLength: 5 },
      items: { type: 'array', minItems: 2, items: { type: 'integer' } }
    }
  }
  assert.deepEqual(validate(schema, { title: 'abcd', items: [1, 2] }), [])
  assert.deepEqual(validate(schema, { title: '一二三四五六', items: [1, 'x'] }), [
    '$.title: "一二三四五六" is too long (6 characters, maximum 5)',
    `$.items[1]: "x" is not of type 'integer'`
  ])
  assert.deepEqual(validate(schema, { items: [] }), ["$: 'title' is a required property", '$.items: should have at least 2 items, has 0'])
  const message = feedbackMessage({ title: 'x' }, Array.from({ length: 12 }, (_, index) => `error ${index}`))
  assert.match(message, /- error 9\n- \.\.\.and 2 more validation errors\./)
  assert.match(message, /Return corrected JSON only/)
})

test('a reply counts only as one finished JSON object', () => {
  assert.deepEqual(replyObject(200, chat({ a: 1 }).body), { value: { a: 1 } })
  assert.deepEqual(replyObject(200, chat('```json\n{"a":1}\n```').body), { value: { a: 1 } })
  assert.deepEqual(replyObject(401, chat({ a: 1 }).body), { error: 'model unavailable (401)' })
  assert.deepEqual(replyObject(200, chat({ a: 1 }, 'length').body), { error: 'model reply was cut off' })
  assert.deepEqual(replyObject(200, chat('[1]').body), { error: 'model reply is not a JSON object' })
  assert.deepEqual(replyObject(200, chat('hello').body), { error: 'model reply is not JSON' })
  assert.deepEqual(replyObject(200, { choices: [] }), { error: 'model reply has no choice' })
})

test('an outline turn returns one Markdown outline per slide from one request', async () => {
  const { ask, bodies } = recorder((body) => fakeReply(body))
  const result = await runStep(outlineParameters, ask)
  assert.equal(result.step, 'outline')
  assert.equal(result.outline.length, 5)
  assert.match(result.outline[0], /^## 量子计算入门/)
  assert.equal(result.requests, 1)
  assert.deepEqual(result.issues, [])
  assert.equal(bodies.length, 1)
  const [system, user] = bodies[0].messages
  assert.match(system.content, /Return only one JSON object that matches this JSON schema:/)
  assert.match(user.content, /Number of Slides: 5\n/)
  assert.match(user.content, /Language: Chinese \(Simplified/)
  assert.doesNotMatch(system.content + user.content, /search|https?:\/\//i)
})

test('an automatic slide count lets the model choose up to the deck limit', async () => {
  const { ask, bodies } = recorder((body) => fakeReply(body))
  const result = await runStep({ ...outlineParameters, n_slides: null }, ask)
  assert.ok(result.outline.length >= 1 && result.outline.length <= MAX_DECK_SLIDES)
  const [system, user] = bodies[0].messages
  assert.match(user.content, /Number of Slides: auto-detect, maximum 20\n/)
  assert.match(system.content, /"minItems":1,"maxItems":20/)
})

test('an invalid reply is sent back with its errors until it validates', async () => {
  const { ask, bodies } = recorder((body, count) => (count === 1 ? chat({ slides: [{ content: '太短' }] }) : fakeReply(body)))
  const result = await runStep({ ...outlineParameters, n_slides: 2 }, ask)
  assert.equal(result.outline.length, 2)
  assert.equal(result.requests, 2)
  const retry = bodies[1].messages
  assert.equal(retry.length, 4)
  assert.equal(retry[2].role, 'assistant')
  assert.match(retry[3].content, /did not match the required response schema/)
  assert.match(retry[3].content, /should have at least 2 items/)
})

test('when fixes run out the last parsed reply is kept with its issues', async () => {
  const short = { slides: [{ content: '## 一\n短' }, { content: '## 二\n短' }] }
  const { ask, bodies } = recorder(() => chat(short))
  const result = await runStep({ ...outlineParameters, n_slides: 2 }, ask)
  assert.equal(bodies.length, MAX_REQUESTS_PER_TURN)
  assert.deepEqual(result.outline, ['## 一\n短', '## 二\n短'])
  assert.match(result.issues[0], /too short/)
})

test('no retry starts after the retry budget', async () => {
  let now = Date.parse('2026-10-13T00:00:00Z')
  const clock = () => new Date(now)
  const { ask, bodies } = recorder(() => {
    now += 50_000
    return chat({ slides: [] })
  })
  await runStep(outlineParameters, ask, clock)
  assert.equal(bodies.length, 1)
})

test('an unavailable model ends the turn with the reason and no retry', async () => {
  const { ask, bodies } = recorder(() => ({ status: 503, body: null }))
  assert.deepEqual(await runStep(outlineParameters, ask), { step: 'outline', error: 'model unavailable (503)', model: DEFAULT_MODEL })
  assert.equal(bodies.length, 1)
})

test('structure picks layout ids, fills bad picks in order, and falls back to the rule', async () => {
  assert.deepEqual(normalizeStructure([2, 99, -1, 'x'], 5, 3), [2, 1, 2, 0, 1])
  const outline = ['## 一', '## 二', '## 三']
  const layouts = TEMPLATES.general.layouts.map((layout) => layout.id)
  const picked = await runStep({ step: 'structure', template: 'general', outline }, async () => chat({ slides: [3, 0, 5] }))
  assert.deepEqual(picked, { step: 'structure', layouts: [layouts[3], layouts[0], layouts[5]], source: 'model', model: DEFAULT_MODEL, requests: 1 })
  const fallback = await runStep({ step: 'structure', template: 'general', outline }, async () => ({ status: 502, body: null }))
  assert.deepEqual(fallback.layouts, layouts.slice(0, 3))
  assert.equal(fallback.source, 'rule')
  assert.equal(fallback.note, 'model unavailable (502)')
})

test('every layout of every template gets schema-valid content in one request', async () => {
  for (const [id, template] of Object.entries(TEMPLATES)) {
    for (const layout of template.layouts) {
      const { ask, bodies } = recorder((body) => fakeReply(body))
      const parameters = { step: 'slide', template: id, layout: layout.id, outline: '## 量子比特\n- 叠加\n- 纠缠', slide_number: 2, language: CHINESE }
      const result = await runStep(parameters, ask)
      assert.equal(result.error, undefined, `${id}/${layout.id}: ${result.error}`)
      assert.equal(result.layout, layout.id)
      assert.equal(bodies.length, layout.schema ? 1 : 0, `${id}/${layout.id}`)
      if (!layout.schema) continue
      assert.deepEqual(result.issues, [], `${id}/${layout.id}`)
      assert.equal(typeof result.content.__speaker_note__, 'string', `${id}/${layout.id}`)
      assert.match(bodies[0].messages[1].content, /# Slide Language:\nChinese \(Simplified/)
    }
  }
})

function command(runId, parameters) {
  return {
    schema: 'aster.code.agent.start.v1',
    request_id: runId,
    identity: { session_id: 'session', run_id: runId },
    prompt: '生成演示文稿的一步。',
    parameters
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
  return { child, post, page: async (after) => (await post('/v1/agent/events:page', { after })).body.events }
}

async function waitFor(agent, matches) {
  const deadline = Date.now() + 5000
  while (Date.now() < deadline) {
    const found = (await agent.page(0)).find(matches)
    if (found) return found
    await new Promise((resolve) => setTimeout(resolve, 10))
  }
  throw new Error('event did not arrive')
}

function send(port, path, body) {
  const payload = JSON.stringify(body)
  return new Promise((resolve, reject) => {
    const request = http.request(
      { host: '127.0.0.1', port, path, method: 'POST', headers: { 'content-type': 'application/json', 'content-length': Buffer.byteLength(payload) } },
      (response) => {
        const chunks = []
        response.on('data', (chunk) => chunks.push(chunk))
        response.on('end', () => {
          assert.ok(response.headers['content-length'], 'the host reads Content-Length replies')
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

test('a turn asks the host for the model and answers with one JSON output', async (t) => {
  const agent = await start()
  t.after(() => agent.child.kill())
  const accepted = await agent.post('/v1/agent/commands', command('deck-1', outlineParameters))
  assert.deepEqual(accepted, { status: 200, body: { accepted: true, run_id: 'deck-1' } })
  assert.equal((await agent.post('/v1/agent/commands', command('deck-1', outlineParameters))).status, 409)

  const request = await waitFor(agent, (event) => event.type === 'model_request')
  assert.equal(request.run_id, 'deck-1')
  assert.equal(request.request, 'deck-1-model-1')
  assert.equal(request.api, 'chat')
  for (const key of Object.keys(request.body)) assert.ok(HOST_MODEL_FIELDS.includes(key), key)
  assert.equal(request.body.model, DEFAULT_MODEL)

  assert.equal((await agent.post('/v1/agent/model-responses', { request: 'deck-9-model-1', status: 200, body: {} })).status, 404)
  const answer = fakeReply(request.body)
  assert.equal((await agent.post('/v1/agent/model-responses', { request: request.request, ...answer })).status, 200)

  await waitFor(agent, (event) => event.run_id === 'deck-1' && event.type === 'completed')
  const events = (await agent.page(0)).filter((event) => event.run_id === 'deck-1')
  assert.deepEqual(events.map((event) => event.type), ['model_request', 'output', 'completed'])
  const output = JSON.parse(events[1].text)
  assert.equal(output.step, 'outline')
  assert.equal(output.outline.length, 5)
  assert.ok(events.every((event, index) => index === 0 || event.sequence > events[index - 1].sequence))
})

test('a bad step completes with an error output instead of failing the box', async (t) => {
  const agent = await start()
  t.after(() => agent.child.kill())
  assert.equal((await agent.post('/v1/agent/commands', { prompt: 'x', identity: {} })).status, 400)
  await agent.post('/v1/agent/commands', command('bad-1', { step: 'outline', n_slides: 99, content: 'x', language: ENGLISH }))
  await waitFor(agent, (event) => event.run_id === 'bad-1' && event.type === 'completed')
  const output = (await agent.page(0)).find((event) => event.run_id === 'bad-1' && event.type === 'output')
  assert.deepEqual(JSON.parse(output.text), { error: 'n_slides must be null or an integer from 1 to 20' })
})
