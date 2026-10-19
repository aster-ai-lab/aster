import readline from 'node:readline'

const sessions = new Map()
const cancelled = new Set()

function emit(message) {
  process.stdout.write(`${JSON.stringify(message)}\n`)
}

function result(id, value) {
  emit({ jsonrpc: '2.0', id, result: value })
}

function failure(id, code, message) {
  emit({ jsonrpc: '2.0', id, error: { code, message } })
}

function notify(method, params) {
  emit({ jsonrpc: '2.0', method, params })
}

function promptText(params) {
  const blocks = Array.isArray(params?.prompt) ? params.prompt : []
  return blocks
    .filter((block) => block && block.type === 'text' && typeof block.text === 'string')
    .map((block) => block.text)
    .join('')
}

function chunks(text) {
  const size = 8
  const parts = []
  for (let index = 0; index < text.length; index += size) parts.push(text.slice(index, index + size))
  return parts.length > 0 ? parts : ['']
}

function stream(sessionId, text) {
  for (const piece of chunks(text)) {
    if (cancelled.has(sessionId)) return false
    notify('session/update', {
      sessionId,
      update: {
        sessionUpdate: 'agent_message_chunk',
        content: { type: 'text', text: piece }
      }
    })
  }
  return true
}

function replyFor(history, latest) {
  if (history.length <= 1) return `收到：${latest}`
  return `上一轮是${history[history.length - 2]}。这一轮是${latest}`
}

function handle(message) {
  const { id, method, params } = message
  if (method === 'initialize') {
    result(id, {
      protocolVersion: 1,
      agentCapabilities: { loadSession: true },
      authMethods: []
    })
    return
  }
  if (method === 'session/new') {
    const sessionId = `sess-${sessions.size + 1}`
    sessions.set(sessionId, { history: [] })
    result(id, { sessionId })
    return
  }
  if (method === 'session/load') {
    const sessionId = String(params?.sessionId ?? '')
    if (!sessionId) {
      failure(id, -32602, 'sessionId is required')
      return
    }
    if (!sessions.has(sessionId)) sessions.set(sessionId, { history: [] })
    const session = sessions.get(sessionId)
    for (const turn of session.history) {
      if (!stream(sessionId, `回放：${turn}`)) break
    }
    result(id, { sessionId })
    return
  }
  if (method === 'session/prompt') {
    const sessionId = String(params?.sessionId ?? '')
    const session = sessions.get(sessionId)
    if (!session) {
      failure(id, -32002, 'session was not found')
      return
    }
    cancelled.delete(sessionId)
    const latest = promptText(params)
    session.history.push(latest)
    const finished = stream(sessionId, replyFor(session.history, latest))
    result(id, { stopReason: finished ? 'end_turn' : 'cancelled' })
    return
  }
  if (method === 'session/cancel') {
    const sessionId = String(params?.sessionId ?? '')
    if (sessionId) cancelled.add(sessionId)
    return
  }
  if (id !== undefined) failure(id, -32601, `unknown method ${method}`)
}

const input = readline.createInterface({ input: process.stdin })
input.on('line', (line) => {
  const trimmed = line.trim()
  if (!trimmed) return
  let message
  try {
    message = JSON.parse(trimmed)
  } catch {
    return
  }
  handle(message)
})
