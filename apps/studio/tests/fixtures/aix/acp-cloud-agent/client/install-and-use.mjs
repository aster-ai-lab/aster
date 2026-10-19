import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { createInterface } from 'node:readline'
import { fileURLToPath } from 'node:url'

const packagedAgent = fileURLToPath(new URL('../agent/acp-agent.mjs', import.meta.url))
const fixtureAgent = fileURLToPath(new URL('../agent.mjs', import.meta.url))
const agentPath = existsSync(packagedAgent) ? packagedAgent : fixtureAgent
const child = spawn(process.execPath, [agentPath], { stdio: ['pipe', 'pipe', 'inherit'] })
const lines = createInterface({ input: child.stdout })
const queue = []
const waiters = []
lines.on('line', (line) => {
  const message = JSON.parse(line)
  const waiter = waiters.shift()
  if (waiter) waiter(message)
  else queue.push(message)
})
const next = () => (queue.length > 0 ? Promise.resolve(queue.shift()) : new Promise((resolve) => waiters.push(resolve)))
let sequence = 0

async function request(method, params) {
  const id = ++sequence
  const updates = []
  child.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`)
  for (;;) {
    const message = await next()
    if (message.id === id) return { response: message, updates }
    if (message.method === 'session/update') updates.push(message)
  }
}

const initialized = await request('initialize', { protocolVersion: 1 })
if (initialized.response.result?.protocolVersion !== 1) {
  throw new Error('ACP initialize rejected protocolVersion 1')
}
const opened = await request('session/new', { cwd: '/workspace', mcpServers: [] })
const sessionId = opened.response.result?.sessionId
if (typeof sessionId !== 'string' || sessionId.length === 0) {
  throw new Error('ACP session/new did not return a sessionId')
}

async function turn(text) {
  const reply = await request('session/prompt', {
    sessionId,
    prompt: [{ type: 'text', text }]
  })
  const streamed = reply.updates.map((update) => update.params?.update?.content?.text ?? '').join('')
  process.stdout.write(`${streamed}\n`)
  if (reply.response.result?.stopReason !== 'end_turn') {
    throw new Error(`ACP turn stopped with ${reply.response.result?.stopReason}`)
  }
}

await turn('第一句')
await turn('第二句')
child.stdin.end()
child.kill()
