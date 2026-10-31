// Homepage seed through the shipped outline, structure, and slide streams.
// The MessagePort is the Studio host: each invoke runs the shipped step with
// the scripted model. The test does not rebuild those steps.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import nodeTest from 'node:test'

// The file shares one deck and one router. Node runs tests in a file at the
// same time, so a later test was opening /upload under an earlier seed.
let chain = Promise.resolve()
function test(name, options, fn) {
  if (typeof options === 'function') {
    fn = options
    options = undefined
  }
  return nodeTest(name, options, async (t) => {
    const wait = chain
    let done
    chain = new Promise((resolve) => {
      done = resolve
    })
    await wait
    try {
      return await fn(t)
    } finally {
      done()
    }
  })
}
test.before = nodeTest.before
test.after = nodeTest.after
test.skip = nodeTest.skip

import './ts-hooks.mjs'
import { MAX_DECK_SLIDES, normalizeStructure, runStep, slideSchema, validate } from '../agent/deck.mjs'
import { ICON_NAMES } from '../agent/icons.mjs'
import { TEMPLATES } from '../agent/templates.mjs'
import { fakeReply } from './fake-model.mjs'

const CHINESE = 'Chinese (Simplified - 中文, 汉语)'
const PROTOCOL = 1
const TEMPLATE = 'general'
const OPEN = () => true
const NativeChannel = globalThis.MessageChannel
globalThis.MessageChannel = class MessageChannel extends NativeChannel {
  constructor() {
    super()
    const ports = [this.port1, this.port2]
    queueMicrotask(() => {
      for (const port of ports) port.unref?.()
    })
  }
}

const requests = []
const ICON_SET = new Set(ICON_NAMES)
const SKIP_FIELD = new Set(['__speaker_note__', 'image_prompt', 'icon_query', '__icon_url__'])
const MARK = {
  outline: '人写锚点',
  page: '人写锚点-页1',
  note: '人写锚点-备注',
  mid: '人写锚点-大纲中途',
  page3: '人写锚点-页3'
}
let failStructure = false
let failSlideNumber = null
let holdSlideTwo = false
let signalHold = null
let releaseHold = null
let lastStructure = null
let hostPort = null

globalThis.location = { hash: `#aix-document=${'ab'.repeat(32)}` }
globalThis.window = {
  scrollTo() {},
  parent: {
    postMessage(_message, _origin, transfer) {
      const port = transfer?.[0]
      if (!port) return
      hostPort = port
      port.onmessage = async (event) => {
        const data = event.data
        if (!data || data.type !== 'aix.agent.invoke') return
        const parameters = data.parameters
        let result
        try {
          result = await runStep(parameters, async (body) => {
            requests.push({
              step: parameters.step,
              slide_number: parameters.slide_number ?? null,
              outline: parameters.outline ?? null
            })
            if (holdSlideTwo && parameters.step === 'slide' && parameters.slide_number === 2) {
              // Slide 1 is stored before this call. The human edits while it waits.
              holdSlideTwo = false
              signalHold?.()
              signalHold = null
              await new Promise((resolve) => {
                releaseHold = resolve
              })
            }
            if (failStructure && parameters.step === 'structure') return { status: 502, body: null }
            if (failSlideNumber != null && parameters.step === 'slide' && parameters.slide_number === failSlideNumber) {
              return { status: 503, body: null }
            }
            return fakeReply(body)
          })
        } catch (error) {
          result = { error: error instanceof Error ? error.message : String(error) }
        }
        if (parameters.step === 'structure') lastStructure = result
        port.postMessage({ protocolVersion: PROTOCOL, type: 'aix.agent.result', id: data.id, text: JSON.stringify(result) })
      }
    }
  }
}

const generate = await import('../view/backend/generate.ts')
const home = await import('../view/backend/homeSeed.ts')
const host = await import('../view/host.ts')
const router = await import('../view/router.ts')
const state = await import('../view/backend/state.ts')
const { slideModel } = await import('../view/backend/deck.ts')
const { viewFetch } = await import('../view/backend/server.ts')
const { template } = await import('../view/backend/templates.ts')
const deckControl = await import('../view/backend/deckControl.ts')

function current() {
  return state.deck
}

async function connectHost() {
  if (host.connected()) return
  const pending = host.connect()
  assert.ok(hostPort, 'the view did not hand the host a port')
  hostPort.postMessage({ protocolVersion: PROTOCOL, type: 'aix.view.connect', stateJSON: null })
  assert.equal(await pending, null)
  assert.equal(host.connected(), true)
}

async function eventsOf(stream) {
  const events = []
  for await (const event of stream) events.push(event)
  return events
}

function releaseSlideHold() {
  const release = releaseHold
  releaseHold = null
  holdSlideTwo = false
  signalHold = null
  if (release) release()
}

function seed(nSlides) {
  releaseSlideHold()
  state.setDeck(null)
  home.resetAcceptedHomeSeedForTests()
  host.resetAgentModelRequestsForTests()
  requests.length = 0
  failStructure = false
  failSlideNumber = null
  lastStructure = null
  const applied = home.acceptHomeSeed({
    content: '给高中生讲量子计算入门',
    n_slides: nSlides,
    language: CHINESE,
    tone: 'educational',
    verbosity: 'standard',
    include_title_slide: true,
    instructions: ''
  })
  assert.equal(applied.applied, true, 'homepage seed was refused')
  assert.equal(host.agentModelRequests, 0, 'a homepage seed called the model')
  assert.equal(requests.length, 0)
  assert.equal(current().outlines, null)
  assert.deepEqual(current().slides, [])
  const here = router.location()
  assert.equal(here.pathname, '/outline', `${here.pathname}${here.search}`)
  assert.equal(new URLSearchParams(here.search).get('id'), current().id)
  return current()
}

function slideTexts(value, skip, out) {
  if (typeof value === 'string') {
    const text = value.trim()
    if (text) out.push(text)
    return
  }
  if (Array.isArray(value)) {
    for (const item of value) slideTexts(item, skip, out)
    return
  }
  if (!value || typeof value !== 'object') return
  for (const [key, child] of Object.entries(value)) {
    if (skip.has(key)) continue
    slideTexts(child, skip, out)
  }
}

function viewDeck(presentation) {
  return {
    id: presentation.id,
    created_at: presentation.created_at,
    updated_at: presentation.updated_at,
    content: presentation.content,
    n_slides: presentation.n_slides,
    language: presentation.language,
    tone: presentation.tone,
    verbosity: presentation.verbosity,
    instructions: presentation.instructions,
    include_title_slide: presentation.include_title_slide,
    title: presentation.title,
    outlines: presentation.outlines,
    template: presentation.template,
    structure: presentation.structure,
    slides: presentation.slides.map((slide) => ({
      id: slide.id,
      group: slide.group,
      layout: slide.layout,
      content: slide.content,
      note: slide.note,
      patch: slide.patch ?? null
    }))
  }
}

function recordFinishedDeck(presentation) {
  const deck = viewDeck(presentation)
  assert.equal(deck.slides.length, presentation.slides.length)
  assert.ok(deck.template)
  assert.ok(deck.slides.every((slide) => slide.layout && slide.group && slide.content))
  if (process.env.ASTER_PPT_DECK_RECORD !== '1') return
  const skip = new Set(['__speaker_note__', 'image_prompt'])
  const slides = presentation.slides.map((slide, index) => {
    const heading = firstHeading(presentation.outlines[index])
    const texts = []
    slideTexts(slide.content, skip, texts)
    const body = texts.filter((text) => text !== heading).join('\n').trim()
    return { heading, body }
  })
  console.log(
    `PPT_DECK ${JSON.stringify({
      id: presentation.id,
      title: presentation.title,
      count: slides.length,
      slides,
      deck
    })}`
  )
}

function firstHeading(markdown) {
  for (const line of markdown.split('\n')) {
    const heading = /^#{1,6}\s+(.+)$/.exec(line.trim())
    if (heading) return heading[1].trim()
  }
  return null
}

function assertOutlineShape(presentation) {
  assert.ok(presentation.outlines?.length >= 1 && presentation.outlines.length <= MAX_DECK_SLIDES)
  for (const item of presentation.outlines) {
    assert.match(item.split('\n')[0], /^#{1,6}\s+\S/, item)
  }
  assert.equal(presentation.title, firstHeading(presentation.outlines[0]))
  assert.ok(presentation.title)
}

function assertFinishedSlides(presentation, templateId) {
  const ids = new Set(template(templateId).layouts.map((layout) => layout.id))
  const agentLayouts = TEMPLATES[templateId].layouts
  assert.equal(presentation.template, templateId)
  assert.equal(presentation.slides.length, presentation.structure.length)
  assert.equal(presentation.slides.length, presentation.outlines.length)
  for (const slide of presentation.slides) {
    assert.equal(slide.group, templateId)
    assert.ok(ids.has(slide.layout), `${slide.layout} is not in ${templateId}`)
    const layout = agentLayouts.find((entry) => entry.id === slide.layout)
    assert.ok(layout, slide.layout)
    const schema = slideSchema(layout, presentation.language)
    assert.ok(schema, slide.layout)
    assert.deepEqual(validate(schema, slide.content), [], slide.layout)
    assert.equal(slide.note, slide.content.__speaker_note__)
    assert.equal(typeof slide.note, 'string')
    assert.ok(slide.note.length > 0, slide.layout)
  }
}

async function outlineOf(id) {
  const events = await eventsOf(generate.outlineStream(id, OPEN))
  assert.equal(events.some((event) => event.type === 'error'), false, JSON.stringify(events.at(-1)))
  assert.equal(events.at(-1)?.type, 'complete')
  const presentation = current()
  assertOutlineShape(presentation)
  return { events, presentation }
}

async function prepareOf(presentation, templateId) {
  const reply = await generate.prepare({
    presentation_id: presentation.id,
    outlines: presentation.outlines.map((content) => ({ content })),
    layout: templateId,
    title: presentation.title
  })
  assert.equal(reply.status, 200, JSON.stringify(reply.body))
  return current()
}

async function slidesOf(id) {
  const events = await eventsOf(generate.slideStream(id, OPEN))
  assert.equal(events.some((event) => event.type === 'error'), false, JSON.stringify(events.find((event) => event.type === 'error')))
  assert.equal(events.at(-1)?.type, 'complete')
  return current()
}

test.before(connectHost)

test('a homepage seed opens the outline page for that deck', () => {
  const created = seed(4)
  const here = router.location()
  assert.equal(here.pathname, '/outline')
  assert.equal(new URLSearchParams(here.search).get('id'), created.id)
  const bundle = readFileSync(new URL('../ui/workbench/app.js', import.meta.url), 'utf8')
  const seeded = /outlines:null,template:null,structure:null,slides:\[\]\}.{0,80}\/outline\?id=/
  const editor = /outlines:null,template:null,structure:null,slides:\[\]\}.{0,80}\/presentation\?id=/
  assert.match(bundle, seeded)
  assert.doesNotMatch(bundle, editor)
})

test('a fixed page count from a homepage seed finishes as a schema-valid deck', async () => {
  const created = seed(5)
  const { presentation } = await outlineOf(created.id)
  assert.equal(presentation.outlines.length, 5)
  assert.equal(presentation.n_slides, 5)
  const prepared = await prepareOf(presentation, TEMPLATE)
  for (const layoutId of prepared.structure) {
    assert.ok(template(TEMPLATE).layouts.some((layout) => layout.id === layoutId), layoutId)
  }
  const finished = await slidesOf(prepared.id)
  assertFinishedSlides(finished, TEMPLATE)
  recordFinishedDeck(finished)
})

test('replaying an existing outline does not send another model request', async () => {
  const created = seed(3)
  const { presentation } = await outlineOf(created.id)
  const requestsBefore = requests.length
  const turnsBefore = host.agentModelRequests
  const events = await eventsOf(generate.outlineStream(presentation.id, OPEN))
  assert.equal(requests.length, requestsBefore)
  assert.equal(host.agentModelRequests, turnsBefore)
  assert.equal(events.some((event) => event.type === 'status'), false)
  assert.equal(events[0]?.type, 'chunk')
  assert.equal(events.at(-1)?.type, 'complete')
  assert.deepEqual(current().outlines, presentation.outlines)
})

test('an empty page count stays within 1 to 20 and still finishes the deck', async () => {
  const created = seed(null)
  assert.equal(created.n_slides, 0)
  const { presentation } = await outlineOf(created.id)
  assert.ok(presentation.outlines.length >= 1 && presentation.outlines.length <= MAX_DECK_SLIDES)
  assert.equal(presentation.n_slides, presentation.outlines.length)
  const prepared = await prepareOf(presentation, TEMPLATE)
  const finished = await slidesOf(prepared.id)
  assertFinishedSlides(finished, TEMPLATE)
})

test('a failed structure step fills layouts by the package rule and slides still generate', async () => {
  const created = seed(4)
  const { presentation } = await outlineOf(created.id)
  failStructure = true
  const prepared = await prepareOf(presentation, TEMPLATE)
  const layoutIds = TEMPLATES[TEMPLATE].layouts.map((layout) => layout.id)
  const byRule = normalizeStructure(null, presentation.outlines.length, layoutIds.length).map((index) => layoutIds[index])
  assert.equal(lastStructure?.source, 'rule')
  assert.deepEqual(prepared.structure, byRule)
  assert.deepEqual(prepared.slides, [])
  const finished = await slidesOf(prepared.id)
  assertFinishedSlides(finished, TEMPLATE)
  assert.deepEqual(finished.structure, byRule)
})

test('a later slide failure keeps earlier slides and a retry continues at the failed slide', async () => {
  const created = seed(3)
  const { presentation } = await outlineOf(created.id)
  const prepared = await prepareOf(presentation, TEMPLATE)
  failSlideNumber = 2
  const failed = await eventsOf(generate.slideStream(prepared.id, OPEN))
  const kept = current()
  assert.equal(failed.some((event) => event.type === 'error'), true)
  assert.equal(failed.some((event) => event.type === 'complete'), false)
  assert.equal(kept.slides.length, 1)
  assert.equal(kept.slides[0].layout, kept.structure[0])
  const keptId = kept.slides[0].id
  const requestsBefore = requests.length
  failSlideNumber = null
  const resumed = await slidesOf(kept.id)
  const added = requests.slice(requestsBefore)
  assert.deepEqual(
    added.filter((entry) => entry.step === 'slide').map((entry) => entry.slide_number),
    [2, 3]
  )
  assert.equal(resumed.slides.length, 3)
  assert.equal(resumed.slides[0].id, keptId)
  assertFinishedSlides(resumed, TEMPLATE)
})

test('regenerating a finished deck replaces those slides', async () => {
  const created = seed(3)
  const { presentation } = await outlineOf(created.id)
  const prepared = await prepareOf(presentation, TEMPLATE)
  const finished = await slidesOf(prepared.id)
  const previous = finished.slides.map((slide) => slide.id)
  const requestsBefore = requests.length
  const replaced = await slidesOf(finished.id)
  const added = requests.slice(requestsBefore).filter((entry) => entry.step === 'slide')
  assert.equal(replaced.slides.length, previous.length)
  assert.equal(added.length, previous.length)
  assert.deepEqual(
    added.map((entry) => entry.slide_number),
    previous.map((_id, index) => index + 1)
  )
  for (const slide of replaced.slides) assert.equal(previous.includes(slide.id), false)
  assertFinishedSlides(replaced, TEMPLATE)
})

function marked(slide) {
  return JSON.stringify(slide.content).includes('人写锚点') || String(slide.note).includes('人写锚点')
}

function markerKey(value, marker) {
  let found = null
  const visit = (node, key) => {
    if (found || node == null) return
    if (typeof node === 'string') {
      if (node.includes(marker)) found = key
      return
    }
    if (typeof node !== 'object') return
    if (Array.isArray(node)) {
      for (const child of node) visit(child, key)
      return
    }
    for (const [childKey, child] of Object.entries(node)) visit(child, childKey)
  }
  visit(value, '')
  return found
}

function appendFirstText(value, marker) {
  let previous = null
  const visit = (node) => {
    if (previous || !node || typeof node !== 'object') return
    if (Array.isArray(node)) {
      for (let index = 0; index < node.length; index += 1) {
        const child = node[index]
        if (typeof child === 'string') {
          if (!child.trim() || ICON_SET.has(child)) continue
          previous = child
          node[index] = child + marker
          return
        }
        visit(child)
        if (previous) return
      }
      return
    }
    for (const [key, child] of Object.entries(node)) {
      if (SKIP_FIELD.has(key)) continue
      if (typeof child === 'string') {
        if (!child.trim() || ICON_SET.has(child)) continue
        previous = child
        node[key] = child + marker
        return
      }
      visit(child)
      if (previous) return
    }
  }
  visit(value)
  return previous
}

function replaceExact(value, previous, next) {
  let done = false
  const visit = (node) => {
    if (done || !node || typeof node !== 'object') return
    if (Array.isArray(node)) {
      for (let index = 0; index < node.length; index += 1) {
        if (node[index] === previous) {
          node[index] = next
          done = true
          return
        }
        visit(node[index])
      }
      return
    }
    for (const [key, child] of Object.entries(node)) {
      if (child === previous) {
        node[key] = next
        done = true
        return
      }
      visit(child)
    }
  }
  visit(value)
}

async function jsonFetch(path, method, body) {
  const reply = await viewFetch(path, { method, body: JSON.stringify(body) })
  return { status: reply.status, body: await reply.json() }
}

async function putOutlines(id, outlines) {
  const reply = await jsonFetch(`/api/v1/ppt/outlines/${id}`, 'PUT', {
    slides: outlines.map((content) => ({ content }))
  })
  assert.equal(reply.status, 200, JSON.stringify(reply.body))
  assert.deepEqual(current().outlines, outlines)
  assert.equal(current().n_slides, outlines.length)
  return current()
}

async function prepareByHand(presentation, templateId) {
  const reply = await jsonFetch('/api/v1/ppt/presentation/prepare', 'POST', {
    presentation_id: presentation.id,
    outlines: presentation.outlines.map((content) => ({ content })),
    layout: templateId,
    title: presentation.title
  })
  assert.equal(reply.status, 200, JSON.stringify(reply.body))
  return current()
}

async function patchSlideText(index, marker, note) {
  const deck = current()
  const before = deck.slides.map((slide) => ({
    id: slide.id,
    content: structuredClone(slide.content),
    note: slide.note
  }))
  const payload = structuredClone(slideModel(deck, deck.slides[index], index))
  const previous = appendFirstText(payload.content, marker)
  assert.ok(previous, `slide ${index + 1} has no visible text`)
  if (payload.ui) replaceExact(payload.ui, previous, previous + marker)
  if (note != null) payload.speaker_note = note
  const reply = await jsonFetch('/api/v1/ppt/presentation/slide_update', 'PATCH', { slide: payload })
  assert.equal(reply.status, 200, JSON.stringify(reply.body))
  const saved = current()
  assert.equal(saved.slides[index].id, before[index].id)
  assert.equal(JSON.stringify(saved.slides[index].content).includes(marker), true)
  assert.equal(SKIP_FIELD.has(markerKey(saved.slides[index].content, marker)), false)
  if (note != null) assert.equal(saved.slides[index].note, note)
  for (let at = 0; at < before.length; at += 1) {
    if (at === index) continue
    assert.equal(saved.slides[at].id, before[at].id)
    assert.deepEqual(saved.slides[at].content, before[at].content)
    assert.equal(saved.slides[at].note, before[at].note)
  }
  return saved
}

async function throughPrepared(failRule) {
  const created = seed(null)
  assert.equal(created.n_slides, 0)
  assert.equal(requests.length, 0)
  const id = created.id
  const { presentation } = await outlineOf(id)
  assert.deepEqual(
    requests.map((entry) => entry.step),
    ['outline']
  )
  assert.equal(presentation.id, id)
  assert.ok(presentation.outlines.length >= 1 && presentation.outlines.length <= MAX_DECK_SLIDES)
  assert.ok(presentation.outlines.length >= 3)
  assert.equal(presentation.outlines.some((item) => item.includes('人写锚点')), false)
  const edited = presentation.outlines.slice(0, -1)
  edited[1] = `${edited[1]}${MARK.outline}`
  await putOutlines(id, edited)
  assert.deepEqual(
    requests.map((entry) => entry.step),
    ['outline']
  )
  const replay = await eventsOf(generate.outlineStream(id, OPEN))
  assert.deepEqual(
    requests.map((entry) => entry.step),
    ['outline']
  )
  assert.equal(replay.some((event) => event.type === 'status'), false)
  assert.equal(replay.at(-1)?.type, 'complete')
  assert.equal(replay.find((event) => event.type === 'chunk')?.chunk.includes(MARK.outline), true)
  assert.deepEqual(current().outlines, edited)
  if (failRule) failStructure = true
  const beforePrepare = requests.length
  const prepared = await prepareByHand(current(), TEMPLATE)
  failStructure = false
  assert.deepEqual(requests.slice(beforePrepare).map((entry) => entry.step), ['structure'])
  assert.deepEqual(requests.at(-1).outline, edited)
  assert.deepEqual(prepared.outlines, edited)
  assert.deepEqual(prepared.slides, [])
  assert.equal(prepared.structure.length, edited.length)
  assert.equal(prepared.template, TEMPLATE)
  assert.equal(prepared.id, id)
  const allowed = new Set(template(TEMPLATE).layouts.map((layout) => layout.id))
  assert.ok(prepared.structure.every((layoutId) => allowed.has(layoutId)))
  if (failRule) {
    const layoutIds = TEMPLATES[TEMPLATE].layouts.map((layout) => layout.id)
    const byRule = normalizeStructure(null, edited.length, layoutIds.length).map((index) => layoutIds[index])
    assert.equal(lastStructure?.source, 'rule')
    assert.equal(lastStructure?.error, undefined)
    assert.deepEqual(prepared.structure, byRule)
    assert.equal(prepared.outlines[1].includes(MARK.outline), true)
  }
  return prepared
}

function beginSlideStream(id) {
  const events = []
  let resolveEntered
  const entered = new Promise((resolve) => {
    resolveEntered = resolve
  })
  signalHold = resolveEntered
  holdSlideTwo = true
  const done = (async () => {
    for await (const event of generate.slideStream(id, OPEN)) events.push(event)
    return events
  })()
  return { entered, done }
}

async function pauseAfterFirstSlide(prepared) {
  const captured = prepared.outlines.slice()
  const before = requests.length
  const run = beginSlideStream(prepared.id)
  let timer
  try {
    await Promise.race([
      run.entered,
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error('slide 2 call did not start')), 5000)
      })
    ])
  } catch (error) {
    releaseSlideHold()
    throw error
  } finally {
    clearTimeout(timer)
  }
  const paused = current()
  if (paused?.slides.length !== 1) {
    releaseSlideHold()
    assert.fail(`pause is not after the first stored slide (${paused?.slides.length ?? 0})`)
  }
  const calls = requests.slice(before).filter((entry) => entry.step === 'slide')
  assert.deepEqual(
    calls.map((entry) => entry.slide_number),
    [1, 2]
  )
  assert.equal(calls[1].outline, captured[1])
  assert.equal(paused.slides[0].layout, paused.structure[0])
  assert.ok(paused.slides[0].id)
  assert.ok(paused.slides[0].content)
  return { captured, before, run, firstId: paused.slides[0].id }
}

function assertStreamUsedCapturedOutlines(before, captured) {
  const calls = requests.slice(before).filter((entry) => entry.step === 'slide')
  assert.deepEqual(
    calls.map((entry) => entry.slide_number),
    captured.map((_outline, index) => index + 1)
  )
  for (const call of calls) assert.equal(call.outline, captured[call.slide_number - 1])
  assert.equal(calls.filter((entry) => entry.slide_number === 1).length, 1)
}

async function finishRemainingSlides(prepared, paused) {
  const mid = paused.captured.slice()
  mid[0] = `${mid[0]}${MARK.mid}`
  await putOutlines(prepared.id, mid)
  releaseSlideHold()
  const events = await paused.run.done
  assert.equal(events.some((event) => event.type === 'error'), false, JSON.stringify(events.find((event) => event.type === 'error')))
  assert.equal(events.at(-1)?.type, 'complete')
  const finished = current()
  assertStreamUsedCapturedOutlines(paused.before, paused.captured)
  assert.equal(finished.id, prepared.id)
  assert.equal(finished.slides.length, paused.captured.length)
  assert.equal(finished.slides[0].id, paused.firstId)
  assert.equal(JSON.stringify(finished.slides[0].content).includes(MARK.page), true)
  assert.equal(finished.slides[0].note, MARK.note)
  for (const slide of finished.slides.slice(1)) assert.equal(marked(slide), false)
  assert.deepEqual(finished.outlines, mid)
  assert.equal(finished.outlines[0].includes(MARK.mid), true)
  for (const call of requests.slice(paused.before).filter((entry) => entry.step === 'slide')) {
    assert.equal(String(call.outline).includes(MARK.mid), false)
  }
  return finished
}

async function editThirdPage(finished) {
  assert.ok(finished.slides.length >= 3)
  const ids = finished.slides.map((slide) => slide.id)
  await patchSlideText(2, MARK.page3, null)
  const edited = current()
  assert.deepEqual(
    edited.slides.map((slide) => slide.id),
    ids
  )
  assert.equal(JSON.stringify(edited.slides[0].content).includes(MARK.page), true)
  assert.equal(edited.slides[0].note, MARK.note)
  assert.equal(JSON.stringify(edited.slides[2].content).includes(MARK.page3), true)
  return edited
}

test('PPT-DUAL', async () => {
  const prepared = await throughPrepared(false)
  try {
    const paused = await pauseAfterFirstSlide(prepared)
    await patchSlideText(0, MARK.page, MARK.note)
    const finished = await finishRemainingSlides(prepared, paused)
    const edited = await editThirdPage(finished)
    const previous = edited.slides.map((slide) => slide.id)
    const before = requests.length
    const replaced = await slidesOf(edited.id)
    const added = requests.slice(before).filter((entry) => entry.step === 'slide')
    assert.equal(added.length, replaced.structure.length)
    assert.deepEqual(
      added.map((entry) => entry.slide_number),
      replaced.structure.map((_layout, index) => index + 1)
    )
    assert.equal(replaced.slides.length, previous.length)
    for (const slide of replaced.slides) {
      assert.equal(previous.includes(slide.id), false)
      assert.equal(marked(slide), false)
    }
    assert.equal(replaced.outlines[0].includes(MARK.mid), true)
    assert.equal(replaced.outlines[1].includes(MARK.outline), true)
    assert.equal(replaced.id, prepared.id)
  } finally {
    releaseSlideHold()
  }
})

test('PPT-DUAL-FAIL', async () => {
  const prepared = await throughPrepared(false)
  try {
    const paused = await pauseAfterFirstSlide(prepared)
    await patchSlideText(0, MARK.page, MARK.note)
    failSlideNumber = 2
    releaseSlideHold()
    const events = await paused.run.done
    assert.equal(events.some((event) => event.type === 'error'), true)
    assert.equal(events.some((event) => event.type === 'complete'), false)
    const kept = current()
    assert.equal(kept.slides.length, 1)
    assert.equal(kept.slides[0].id, paused.firstId)
    assert.equal(JSON.stringify(kept.slides[0].content).includes(MARK.page), true)
    assert.equal(kept.slides[0].note, MARK.note)
    const before = requests.length
    failSlideNumber = null
    const resumed = await slidesOf(kept.id)
    const added = requests.slice(before).filter((entry) => entry.step === 'slide')
    assert.deepEqual(
      added.map((entry) => entry.slide_number),
      kept.structure.slice(1).map((_layout, index) => index + 2)
    )
    assert.equal(resumed.slides[0].id, paused.firstId)
    assert.equal(JSON.stringify(resumed.slides[0].content).includes(MARK.page), true)
    assert.equal(resumed.slides[0].note, MARK.note)
    assert.equal(resumed.id, prepared.id)
  } finally {
    failSlideNumber = null
    releaseSlideHold()
  }
})

test('PPT-DUAL-KEEP', async () => {
  const prepared = await throughPrepared(false)
  try {
    const paused = await pauseAfterFirstSlide(prepared)
    await patchSlideText(0, MARK.page, MARK.note)
    const finished = await finishRemainingSlides(prepared, paused)
    const beforeRefusal = {
      outlines: finished.outlines.slice(),
      content: structuredClone(finished.slides[0].content),
      note: finished.slides[0].note
    }
    const refused = home.acceptHomeSeed({
      content: '另一主题：经典力学',
      n_slides: null,
      language: CHINESE,
      tone: 'educational',
      verbosity: 'standard',
      include_title_slide: true,
      instructions: ''
    })
    assert.equal(refused.applied, false)
    assert.equal(refused.notice, home.HOME_SEED_KEPT_NOTICE)
    const kept = current()
    assert.equal(kept.id, finished.id)
    assert.equal(kept.slides.length, finished.slides.length)
    assert.deepEqual(kept.outlines, beforeRefusal.outlines)
    assert.deepEqual(kept.slides[0].content, beforeRefusal.content)
    assert.equal(kept.slides[0].note, beforeRefusal.note)
    assert.equal(JSON.stringify(kept.slides[0].content).includes(MARK.page), true)
    assert.equal(kept.outlines[0].includes(MARK.mid), true)
    assert.equal(kept.outlines[1].includes(MARK.outline), true)
    console.log(`PPT-DUAL-KEEP human=${MARK.page} notice=${refused.notice}`)
  } finally {
    releaseSlideHold()
  }
})

test('PPT-DUAL-RULE', async () => {
  const prepared = await throughPrepared(true)
  try {
    const paused = await pauseAfterFirstSlide(prepared)
    await patchSlideText(0, MARK.page, MARK.note)
    const finished = await finishRemainingSlides(prepared, paused)
    await editThirdPage(finished)
    const saved = current()
    assert.equal(saved.id, prepared.id)
    assert.equal(saved.outlines[1].includes(MARK.outline), true)
    assert.equal(saved.slides[0].id, paused.firstId)
    assert.equal(JSON.stringify(saved.slides[0].content).includes(MARK.page), true)
    assert.equal(saved.slides[0].note, MARK.note)
  } finally {
    releaseSlideHold()
  }
})

function toolPart(name, input, id) {
  return { type: 'dynamic-tool', toolName: name, toolCallId: id, state: 'output-available', input }
}

function resetHarness() {
  releaseSlideHold()
  state.setDeck(null)
  home.resetAcceptedHomeSeedForTests()
  host.resetAgentModelRequestsForTests()
  deckControl.resetDeckControlForTests()
  requests.length = 0
  failStructure = false
  failSlideNumber = null
  lastStructure = null
}

function deckFromTool(nSlides) {
  resetHarness()
  const result = deckControl.applyDeckToolPart(
    toolPart(
      'mcp__presenton__generate_deck',
      {
        content: '给高中生讲量子计算入门',
        n_slides: nSlides,
        language: CHINESE,
        tone: 'educational',
        verbosity: 'standard',
        include_title_slide: true,
        instructions: ''
      },
      'gen-1'
    )
  )
  assert.equal(result.status, 'applied', JSON.stringify(result))
  assert.equal(current().content, '给高中生讲量子计算入门')
  assert.equal(current().outlines, null)
  assert.deepEqual(current().slides, [])
  return current()
}

const MARKDOWN_SOURCE = '量子计算的**重点说明**'
const MARKDOWN_RUN = '量子计算的重点说明'

function setFirstVisible(value, next) {
  let previous = null
  const visit = (node) => {
    if (previous || !node || typeof node !== 'object') return
    if (Array.isArray(node)) {
      for (let index = 0; index < node.length; index += 1) {
        const child = node[index]
        if (typeof child === 'string') {
          if (!child.trim() || ICON_SET.has(child)) continue
          previous = child
          node[index] = next
          return
        }
        visit(child)
        if (previous) return
      }
      return
    }
    for (const [key, child] of Object.entries(node)) {
      if (SKIP_FIELD.has(key)) continue
      if (typeof child === 'string') {
        if (!child.trim() || ICON_SET.has(child)) continue
        previous = child
        node[key] = next
        return
      }
      visit(child)
      if (previous) return
    }
  }
  visit(value)
  return previous
}

function renderedRuns(deck, index) {
  const model = slideModel(deck, deck.slides[index], index)
  const texts = []
  const visit = (node) => {
    if (!node || typeof node !== 'object') return
    if (Array.isArray(node)) {
      for (const item of node) visit(item)
      return
    }
    if (Array.isArray(node.runs)) {
      for (const run of node.runs) if (run && typeof run.text === 'string') texts.push(run.text)
    }
    for (const [key, value] of Object.entries(node)) {
      if (key === 'runs' || !value || typeof value !== 'object') continue
      visit(value)
    }
  }
  visit(model.ui)
  return texts
}

/** The first stored field is the markdown shape the slide step requires. Rendered runs drop the markers. */
function plantMarkdownSlide(index) {
  const deck = current()
  const content = structuredClone(deck.slides[index].content)
  assert.ok(setFirstVisible(content, MARKDOWN_SOURCE), `slide ${index + 1} has no visible text`)
  const slides = deck.slides.map((slide, at) => (at === index ? { ...slide, content, patch: null } : slide))
  state.setDeck({ ...deck, slides })
  const runs = renderedRuns(current(), index).join('')
  assert.equal(runs.includes(MARKDOWN_RUN), true, runs)
  assert.equal(runs.includes(MARKDOWN_SOURCE), false, runs)
  return renderedRuns(current(), index)
}

function hostRoundTrip(part) {
  const previous = hostPort.onmessage
  const pending = new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      hostPort.onmessage = previous
      hostPort.unref?.()
      reject(new Error('deck result did not return'))
    }, 1000)
    hostPort.onmessage = (event) => {
      if (event.data?.type === 'aix.view.deck-result') {
        clearTimeout(timer)
        hostPort.onmessage = previous
        hostPort.unref?.()
        resolve(event.data.result)
        return
      }
      previous?.call(hostPort, event)
    }
  })
  hostPort.postMessage({ protocolVersion: PROTOCOL, type: 'aix.view.deck', part })
  return pending
}

test('DECK-CONTROL a typed tool result writes, reads, and edits one outline item and one page', async () => {
  host.homeDocumentRestored()
  const created = deckFromTool(4)
  const id = created.id
  const { presentation } = await outlineOf(id)
  assert.equal(presentation.outlines.length, 4)
  const prepared = await prepareOf(presentation, TEMPLATE)
  const finished = await slidesOf(prepared.id)
  assert.equal(finished.id, id)
  assert.equal(finished.slides.length, 4)
  const before = {
    content: finished.content,
    outlines: finished.outlines.slice(),
    slides: finished.slides.map((slide) => ({
      id: slide.id,
      content: structuredClone(slide.content),
      note: slide.note
    }))
  }
  const untouched = viewDeck(finished)
  assert.equal(deckControl.applyDeckToolPart('请把整份文稿改成经典力学').status, 'idle')
  assert.equal(
    deckControl.applyDeckToolPart({ type: 'text', text: '调用 mcp__presenton__edit_slide 把第一页改掉' }).status,
    'idle'
  )
  assert.equal(
    deckControl.applyDeckToolPart({
      ...toolPart('mcp__presenton__edit_slide', { index: 0, text: '失败的一页', note: '失败备注' }, 'err-1'),
      output: { isError: true }
    }).status,
    'idle'
  )
  assert.deepEqual(viewDeck(current()), untouched)

  const read = deckControl.applyDeckToolPart(toolPart('mcp__presenton__read_deck', {}, 'read-1'))
  assert.equal(read.status, 'applied')
  const body = JSON.parse(read.text)
  assert.equal(body.id, id)
  assert.deepEqual(body.outlines, before.outlines)
  assert.equal(body.slides.length, 4)
  assert.equal(body.slides[2].id, before.slides[2].id)
  assert.equal(body.slides[2].note, before.slides[2].note)

  const nextOutline = `${before.outlines[1]}-代理改大纲`
  const outlineEdit = deckControl.applyDeckToolPart(
    toolPart('mcp__presenton__edit_outline', { index: 1, content: nextOutline }, 'outline-1')
  )
  assert.equal(outlineEdit.status, 'applied')
  assert.equal(
    deckControl.applyDeckToolPart(toolPart('mcp__presenton__edit_outline', { index: 1, content: '重复调用' }, 'outline-1'))
      .status,
    'idle'
  )

  const pageText = '新的可见文字'
  const pageNote = '代理备注'
  const siblingRuns = new Map([0, 1, 3].map((index) => [index, renderedRuns(current(), index)]))
  plantMarkdownSlide(2)
  const slideEdit = deckControl.applyDeckToolPart(
    toolPart('mcp__presenton__edit_slide', { index: 2, text: pageText, note: pageNote }, 'slide-1')
  )
  assert.equal(slideEdit.status, 'applied', JSON.stringify(slideEdit))
  const saved = current()
  const runs = renderedRuns(saved, 2)
  assert.equal(runs.join('').includes(pageText), true, runs.join('\n'))
  assert.equal(runs.join('').includes(MARKDOWN_RUN), false, runs.join('\n'))
  assert.equal(runs.join('').includes(MARKDOWN_SOURCE), false, runs.join('\n'))
  for (const index of [0, 1, 3]) assert.deepEqual(renderedRuns(saved, index), siblingRuns.get(index))
  assert.equal(saved.id, id)
  assert.equal(saved.content, before.content)
  assert.equal(saved.outlines[1], nextOutline)
  assert.equal(saved.outlines.includes('重复调用'), false)
  for (const at of [0, 2, 3]) assert.equal(saved.outlines[at], before.outlines[at])
  assert.equal(saved.slides[2].id, before.slides[2].id)
  assert.equal(saved.slides[2].note, pageNote)
  assert.equal(saved.slides[2].content.__speaker_note__, pageNote)
  assert.equal(JSON.stringify(saved.slides[2].content).includes(pageText), true)
  for (const at of [0, 1, 3]) {
    assert.equal(saved.slides[at].id, before.slides[at].id)
    assert.deepEqual(saved.slides[at].content, before.slides[at].content)
    assert.equal(saved.slides[at].note, before.slides[at].note)
  }

  const viaHost = await hostRoundTrip(toolPart('mcp__presenton__read_deck', {}, 'read-port'))
  assert.equal(viaHost.status, 'applied')
  const hosted = JSON.parse(viaHost.text)
  assert.equal(hosted.id, id)
  assert.equal(hosted.outlines[1], nextOutline)
  assert.equal(hosted.outlines[0], before.outlines[0])
  assert.equal(hosted.slides[2].note, pageNote)
  assert.equal(hosted.slides[2].text.includes(pageText), true)
  assert.equal(hosted.slides[0].id, before.slides[0].id)
  assert.equal(hosted.slides[0].note, before.slides[0].note)
  assert.equal(hosted.slides[1].text, body.slides[1].text)
  const bundle = readFileSync(new URL('../ui/workbench/app.js', import.meta.url), 'utf8')
  assert.equal(bundle.includes('aix.view.deck'), true)
  assert.equal(bundle.includes('__asterPresentonApplyDeckToolPart'), true)
  assert.equal(bundle.includes('aix.view.deck-snapshot'), true)
})

test('DECK-CONTROL-DUAL a human page and an agent page both stay when a new topic is refused', async () => {
  const created = deckFromTool(4)
  const id = created.id
  const { presentation } = await outlineOf(id)
  await prepareOf(presentation, TEMPLATE)
  await slidesOf(id)
  await patchSlideText(0, MARK.page, MARK.note)
  const agentText = '新的可见文字'
  const agentNote = '代理另一页备注'
  plantMarkdownSlide(2)
  const edited = deckControl.applyDeckToolPart(
    toolPart('mcp__presenton__edit_slide', { index: 2, text: agentText, note: agentNote }, 'slide-human')
  )
  assert.equal(edited.status, 'applied', JSON.stringify(edited))
  const refused = deckControl.applyDeckToolPart(
    toolPart(
      'mcp__presenton__generate_deck',
      {
        content: '另一主题：经典力学',
        n_slides: 4,
        language: CHINESE,
        tone: 'educational',
        verbosity: 'standard',
        include_title_slide: true,
        instructions: ''
      },
      'gen-2'
    )
  )
  assert.equal(refused.status, 'refused')
  assert.equal(refused.text, home.HOME_SEED_KEPT_NOTICE)
  const kept = current()
  assert.equal(kept.id, id)
  assert.equal(kept.content, '给高中生讲量子计算入门')
  assert.equal(kept.content.includes('经典力学'), false)
  assert.equal(JSON.stringify(kept.slides[0].content).includes(MARK.page), true)
  assert.equal(kept.slides[0].note, MARK.note)
  assert.equal(JSON.stringify(kept.slides[2].content).includes(agentText), true)
  assert.equal(kept.slides[2].note, agentNote)
  const agentRuns = renderedRuns(kept, 2).join('')
  assert.equal(agentRuns.includes(agentText), true, agentRuns)
  assert.equal(agentRuns.includes(MARKDOWN_RUN), false, agentRuns)
  assert.equal(renderedRuns(kept, 0).join('').includes(MARK.page), true)
  assert.equal(kept.slides[1].note === agentNote, false)
  assert.notEqual(kept.slides[0].id, kept.slides[2].id)
  console.log(`DECK-CONTROL-DUAL human=${MARK.page} agent=${agentText} notice=${refused.text}`)
})
