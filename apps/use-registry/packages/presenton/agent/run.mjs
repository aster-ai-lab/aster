// aster.code.agent.v1 edge entry. One warm box serves many turns of one view, so
// the event sequence is process-wide and every event names its run. The box
// holds no credential: a model call is a `model_request` event the host answers.
import http from 'node:http'

import { runStep } from './deck.mjs'

const MAX_BODY = 256 * 1024
const MAX_EVENTS = 512
const MAX_PAGE = 128
const MAX_RUNS = 1024
const MAX_OUTPUT = 60 * 1024
const MODEL_WAIT_MS = 60_000

let sequence = 0
const events = []
const runs = new Set()
const waiting = new Map()

function emit(event) {
  sequence += 1
  events.push({ sequence, ...event })
  if (events.length > MAX_EVENTS) events.splice(0, events.length - MAX_EVENTS)
}

function awaitModel(request) {
  return new Promise((resolve) => {
    const timer = setTimeout(() => {
      waiting.delete(request)
      resolve({ status: 504, body: null })
    }, MODEL_WAIT_MS)
    waiting.set(request, (answer) => {
      clearTimeout(timer)
      resolve(answer)
    })
  })
}

/**
 * A step that cannot run (bad settings, no model) still completes: the output
 * carries `error`, so the view can say what went wrong.
 */
async function run(command) {
  const runId = command.identity.run_id
  let asked = 0
  const ask = (body) => {
    asked += 1
    const request = `${runId}-model-${asked}`
    const answer = awaitModel(request)
    emit({ run_id: runId, type: 'model_request', request, api: 'chat', body })
    return answer
  }
  let output
  try {
    output = await runStep(command.parameters, ask)
  } catch (error) {
    output = { error: String(error?.message ?? error) }
  }
  let text = JSON.stringify(output)
  if (Buffer.byteLength(text) > MAX_OUTPUT) text = JSON.stringify({ step: output.step, error: 'the answer is too large for one turn' })
  emit({ run_id: runId, type: 'output', text })
  emit({ run_id: runId, type: 'completed' })
}

function validCommand(body) {
  const identity = body?.identity
  return (
    typeof body?.prompt === 'string' &&
    identity &&
    typeof identity.run_id === 'string' &&
    identity.run_id.length > 0 &&
    identity.run_id.length <= 128 &&
    (body.parameters === undefined || (body.parameters && typeof body.parameters === 'object'))
  )
}

async function readBody(request) {
  const chunks = []
  let size = 0
  for await (const chunk of request) {
    size += chunk.length
    if (size > MAX_BODY) return undefined
    chunks.push(chunk)
  }
  try {
    return JSON.parse(Buffer.concat(chunks).toString('utf8') || '{}')
  } catch {
    return undefined
  }
}

// The host reads one Content-Length body; it does not decode chunked replies.
function reply(response, status, value) {
  const body = Buffer.from(JSON.stringify(value))
  response.writeHead(status, { 'content-type': 'application/json', 'content-length': body.length })
  response.end(body)
}

const server = http.createServer(async (request, response) => {
  if (request.method !== 'POST') return reply(response, 405, { error: 'method_not_allowed' })
  const body = await readBody(request)
  if (body === undefined) return reply(response, 400, { error: 'invalid_body' })
  if (request.url === '/v1/agent/commands') {
    if (!validCommand(body)) return reply(response, 400, { error: 'invalid_command' })
    if (runs.has(body.identity.run_id)) return reply(response, 409, { error: 'duplicate_run' })
    runs.add(body.identity.run_id)
    if (runs.size > MAX_RUNS) runs.delete(runs.values().next().value)
    reply(response, 200, { accepted: true, run_id: body.identity.run_id })
    run(body).catch((error) => {
      emit({ run_id: body.identity.run_id, type: 'failed', message: String(error?.message ?? error) })
    })
    return
  }
  if (request.url === '/v1/agent/events:page') {
    const after = Number.isSafeInteger(body.after) && body.after >= 0 ? body.after : 0
    return reply(response, 200, { events: events.filter((event) => event.sequence > after).slice(0, MAX_PAGE) })
  }
  if (request.url === '/v1/agent/model-responses') {
    const resolve = typeof body.request === 'string' ? waiting.get(body.request) : undefined
    if (!resolve || !Number.isInteger(body.status)) return reply(response, 404, { error: 'unknown_request' })
    waiting.delete(body.request)
    resolve({ status: body.status, body: body.body ?? null })
    return reply(response, 200, { accepted: true })
  }
  reply(response, 404, { error: 'not_found' })
})

server.listen(0, '127.0.0.1', () => {
  const address = server.address()
  const port = address && typeof address === 'object' ? address.port : 0
  process.stdout.write(`port ${port}\n`)
})
